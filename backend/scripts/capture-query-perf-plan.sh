#!/usr/bin/env bash
set -euo pipefail

readonly EXPECTED_IMAGE_REF='ghcr.io/fohte/t-rader/perf-db:e8715ae2cb74fa0a8a05d65f7af4ccfea9cb7658c56cb9bd2ee604572719922d'
readonly EXPECTED_REPLAY_TARGET_DATE='2026-10-22'
readonly EXPECTED_REPLAY_SQL_COUNT='350'
readonly EXPECTED_APPLIED_EXCLUSION_COUNT='15'
readonly REFERENCE_SOURCE_RUN_ID='39714934810'
readonly REFERENCE_SOURCE_ARTIFACT_ID='12399056019'
readonly REFERENCE_LOCAL_LOG_SHA256='6a095a31d0d22eaf1517c23ae48b374056017ad7eabf26fd882ebf6b6af3320b'
readonly REFERENCE_SOURCE_ZIP_SHA256='50d7b38e3a07a9939cc87f9e1bdbf5084240c9ab13ecd53e1ddb34ad50b499e6'
readonly SOURCE_PR_HEAD_SHA='556c691d809b11afa566533aa98f6a02bf68ef69'
readonly SOURCE_CHECKOUT_SHA='d24a3e71dac02684f1d752c4d37aeb6462ca5c91'
readonly DB_CONTAINER='query-perf-plan-capture-db'
readonly DB_URL='postgres://t_rader:t_rader@127.0.0.1:5432/t_rader'
readonly QUERY_LOG_FILE="${QUERY_LOG_FILE:-$RUNNER_TEMP/backend-query-log-regenerated/postgresql.json}"
readonly INPUT_ARTIFACT_NAME="${INPUT_ARTIFACT_NAME:-}"
readonly INPUT_ARTIFACT_ID="${INPUT_ARTIFACT_ID:-}"
readonly CAPTURE_OUTPUT_DIR="${CAPTURE_OUTPUT_DIR:-query-perf-plan-capture}"
readonly INPUT_MANIFEST_FILE="${INPUT_MANIFEST_FILE:-$CAPTURE_OUTPUT_DIR/input/regenerated-manifest.json}"
readonly INPUT_COMPARISON_FILE="${INPUT_COMPARISON_FILE:-$CAPTURE_OUTPUT_DIR/input/comparison.json}"
readonly INPUT_GROUP_BINDS_FILE="${INPUT_GROUP_BINDS_FILE:-$CAPTURE_OUTPUT_DIR/binds/source-binds.json}"
readonly SOURCE_SQL_PATH='backend/crates/gateways/postgres/src/group_movers.rs'

overall_status=0
replay_status='not-run'
plans_collected=false
container_started=false
mkdir -p "$CAPTURE_OUTPUT_DIR"/{environment,profile,replay,sql,binds,plans,input}

# EXIT trap から呼ぶため、直接の関数呼び出しとしては検出されない。
# shellcheck disable=SC2329
cleanup() {
  local exit_status=$?
  if ((exit_status != 0 && overall_status == 0)); then
    overall_status=$exit_status
  fi
  if [[ "$container_started" == true ]]; then
    docker rm --force "$DB_CONTAINER" > "$CAPTURE_OUTPUT_DIR/environment/container-cleanup.stdout" 2> "$CAPTURE_OUTPUT_DIR/environment/container-cleanup.stderr" || true
  fi
  printf 'script_exit_code=%s\nreplay_exit_code=%s\noverall_status=%s\nplans_collected=%s\n' \
    "$exit_status" "$replay_status" "$overall_status" "$plans_collected" \
    > "$CAPTURE_OUTPUT_DIR/result.txt"
}
trap cleanup EXIT

record_environment() {
  local input_log_sha256='unavailable'
  local input_manifest_sha256='unavailable'
  local input_group_binds_sha256='unavailable'
  local comparison_matches='unavailable'
  if [[ -f "$QUERY_LOG_FILE" ]]; then
    input_log_sha256=$(sha256sum "$QUERY_LOG_FILE" | cut -d ' ' -f 1)
  fi
  if [[ -f "$INPUT_MANIFEST_FILE" ]]; then
    input_manifest_sha256=$(sha256sum "$INPUT_MANIFEST_FILE" | cut -d ' ' -f 1)
  fi
  if [[ -f "$INPUT_GROUP_BINDS_FILE" ]]; then
    input_group_binds_sha256=$(sha256sum "$INPUT_GROUP_BINDS_FILE" | cut -d ' ' -f 1)
  fi
  if [[ -f "$INPUT_COMPARISON_FILE" ]]; then
    comparison_matches=$(jq -r '.matches' "$INPUT_COMPARISON_FILE")
  fi
  {
    printf 'diagnostic_checkout_sha=%s\n' "$(git rev-parse HEAD)"
    printf 'diagnostic_branch=%s\n' "$(git branch --show-current)"
    printf 'source_checkout_sha=%s\n' "$SOURCE_CHECKOUT_SHA"
    printf 'source_pr_head_sha=%s\n' "$SOURCE_PR_HEAD_SHA"
    printf 'source_run_url=https://github.com/fohte/t-rader/actions/runs/%s\n' "$REFERENCE_SOURCE_RUN_ID"
    printf 'reference_source_run_id=%s\n' "$REFERENCE_SOURCE_RUN_ID"
    printf 'reference_source_artifact_id=%s\n' "$REFERENCE_SOURCE_ARTIFACT_ID"
    printf 'reference_local_log_sha256=%s\n' "$REFERENCE_LOCAL_LOG_SHA256"
    printf 'reference_source_zip_sha256=%s\n' "$REFERENCE_SOURCE_ZIP_SHA256"
    printf 'reference_source_zip_identity_proven=false\n'
    printf 'input_kind=ci-regenerated\n'
    printf 'input_artifact_name=%s\n' "$INPUT_ARTIFACT_NAME"
    printf 'input_artifact_id=%s\n' "$INPUT_ARTIFACT_ID"
    printf 'input_log_sha256=%s\n' "$input_log_sha256"
    printf 'input_manifest_sha256=%s\n' "$input_manifest_sha256"
    printf 'input_group_binds_sha256=%s\n' "$input_group_binds_sha256"
    printf 'input_comparison_matches=%s\n' "$comparison_matches"
    printf 'runner_os=%s\n' "$(uname -a)"
    printf 'cpu_count=%s\n' "$(nproc)"
    printf 'image_expected=%s\n' "$EXPECTED_IMAGE_REF"
  } > "$CAPTURE_OUTPUT_DIR/environment/metadata.txt"
  lscpu > "$CAPTURE_OUTPUT_DIR/environment/lscpu.txt" 2>&1 || true
  free -b > "$CAPTURE_OUTPUT_DIR/environment/memory.txt" 2>&1 || true
  docker version > "$CAPTURE_OUTPUT_DIR/environment/docker-version.txt" 2>&1 || true
  git show -s --format='%H%n%P%n%s' HEAD > "$CAPTURE_OUTPUT_DIR/environment/diagnostic-commit.txt"
}

record_environment

if ! backend/scripts/verify-query-perf-plan-capture-source.sh \
  "$CAPTURE_OUTPUT_DIR/environment/source-change-check.txt"; then
  printf 'Diagnostic source ancestry or protected-source check failed.\n' > "$CAPTURE_OUTPUT_DIR/environment/source-check-error.txt"
  exit 1
fi

if [[ -z "$INPUT_ARTIFACT_NAME" || -z "$INPUT_ARTIFACT_ID" ]]; then
  printf 'Regenerated Actions artifact metadata is missing.\n' > "$CAPTURE_OUTPUT_DIR/input/artifact-metadata-error.txt"
  exit 1
fi

if [[ ! -f "$INPUT_MANIFEST_FILE" || ! -f "$INPUT_COMPARISON_FILE" || ! -f "$INPUT_GROUP_BINDS_FILE" ]]; then
  printf 'Hash-only input manifest, comparison report, or source-shifted bind set is missing.\n' > "$CAPTURE_OUTPUT_DIR/input/manifest-error.txt"
  exit 1
fi

if ! jq -e --arg target_date "$EXPECTED_REPLAY_TARGET_DATE" \
  '.provenance.input_kind == "ci-regenerated" and .comparison.target_date == $target_date' \
  "$INPUT_MANIFEST_FILE" > "$CAPTURE_OUTPUT_DIR/input/manifest-validation.stdout" 2> "$CAPTURE_OUTPUT_DIR/input/manifest-validation.stderr" ||
  ! jq -e '.matches == true' "$INPUT_COMPARISON_FILE" \
    > "$CAPTURE_OUTPUT_DIR/input/comparison-validation.stdout" 2> "$CAPTURE_OUTPUT_DIR/input/comparison-validation.stderr"; then
  printf 'Regenerated input comparison did not pass before replay.\n' > "$CAPTURE_OUTPUT_DIR/input/comparison-error.txt"
  exit 1
fi

if [[ ! -f "$QUERY_LOG_FILE" ]]; then
  printf 'Missing regenerated query log: %s\n' "$QUERY_LOG_FILE" > "$CAPTURE_OUTPUT_DIR/replay/source-log-error.txt"
  exit 1
fi

if git diff --quiet "$SOURCE_CHECKOUT_SHA" HEAD -- "$SOURCE_SQL_PATH" backend/crates/gateways/postgres/src/group_query_sql.rs; then
  cp "$SOURCE_SQL_PATH" "$CAPTURE_OUTPUT_DIR/sql/group_movers.rs"
  cp backend/crates/gateways/postgres/src/group_query_sql.rs "$CAPTURE_OUTPUT_DIR/sql/group_query_sql.rs"
  printf 'Application SQL source files match the original checkout.\n' > "$CAPTURE_OUTPUT_DIR/sql/source-files-check.txt"
else
  printf 'Application SQL source files differ from the original checkout.\n' > "$CAPTURE_OUTPUT_DIR/sql/source-files-check.txt"
  exit 1
fi

computed_image_ref=''
if computed_image_ref=$(backend/scripts/build-perf-db-image.sh --image-ref 2> "$CAPTURE_OUTPUT_DIR/environment/image-ref.stderr"); then
  printf 'expected=%s\ncomputed=%s\n' "$EXPECTED_IMAGE_REF" "$computed_image_ref" > "$CAPTURE_OUTPUT_DIR/environment/image-ref-check.txt"
else
  printf 'Failed to compute the image reference.\n' > "$CAPTURE_OUTPUT_DIR/environment/image-ref-error.txt"
  exit 1
fi

if [[ "$computed_image_ref" != "$EXPECTED_IMAGE_REF" ]]; then
  printf 'Image reference mismatch. Expected %s, computed %s.\n' "$EXPECTED_IMAGE_REF" "$computed_image_ref" > "$CAPTURE_OUTPUT_DIR/environment/image-ref-error.txt"
  exit 1
fi

if docker manifest inspect --verbose "$EXPECTED_IMAGE_REF" > "$CAPTURE_OUTPUT_DIR/environment/image-manifest.json" 2> "$CAPTURE_OUTPUT_DIR/environment/image-manifest.stderr"; then
  if docker pull "$EXPECTED_IMAGE_REF" > "$CAPTURE_OUTPUT_DIR/environment/image-pull.stdout" 2> "$CAPTURE_OUTPUT_DIR/environment/image-pull.stderr"; then
    printf 'image_source=registry\n' >> "$CAPTURE_OUTPUT_DIR/environment/image-summary.txt"
  else
    printf 'Failed to pull the expected image.\n' > "$CAPTURE_OUTPUT_DIR/environment/image-error.txt"
    exit 1
  fi
else
  if grep -Eq 'no such manifest|manifest unknown|404 Not Found' \
    "$CAPTURE_OUTPUT_DIR/environment/image-manifest.stderr" \
    "$CAPTURE_OUTPUT_DIR/environment/image-manifest.json"; then
    printf 'image_source=local-build\n' >> "$CAPTURE_OUTPUT_DIR/environment/image-summary.txt"
    if backend/scripts/build-perf-db-image.sh > "$CAPTURE_OUTPUT_DIR/environment/image-build.stdout" 2> "$CAPTURE_OUTPUT_DIR/environment/image-build.stderr"; then
      :
    else
      build_status=$?
      printf 'build_exit_code=%s\n' "$build_status" > "$CAPTURE_OUTPUT_DIR/environment/image-error.txt"
      exit 1
    fi
  else
    printf 'Registry image inspection failed.\n' > "$CAPTURE_OUTPUT_DIR/environment/image-error.txt"
    exit 1
  fi
fi

docker image inspect --format '{{.Id}}' "$EXPECTED_IMAGE_REF" > "$CAPTURE_OUTPUT_DIR/environment/image-id.txt"
docker image inspect --format '{{json .RepoDigests}}' "$EXPECTED_IMAGE_REF" > "$CAPTURE_OUTPUT_DIR/environment/image-digests.json"
docker image inspect --format '{{index .Config.Labels "org.opencontainers.image.source"}}' "$EXPECTED_IMAGE_REF" > "$CAPTURE_OUTPUT_DIR/environment/image-source.txt"
{
  printf 'image_ref=%s\n' "$EXPECTED_IMAGE_REF"
  printf 'image_id=%s\n' "$(< "$CAPTURE_OUTPUT_DIR/environment/image-id.txt")"
  printf 'repo_digests=%s\n' "$(jq -r 'join(",")' "$CAPTURE_OUTPUT_DIR/environment/image-digests.json")"
  printf 'source_label=%s\n' "$(< "$CAPTURE_OUTPUT_DIR/environment/image-source.txt")"
} >> "$CAPTURE_OUTPUT_DIR/environment/image-summary.txt"

if docker run \
  --detach \
  --name "$DB_CONTAINER" \
  --restart=on-failure:5 \
  --memory=6g \
  --memory-swap=6g \
  --publish 127.0.0.1:5432:5432 \
  "$EXPECTED_IMAGE_REF" \
  > "$CAPTURE_OUTPUT_DIR/environment/container-start.stdout" \
  2> "$CAPTURE_OUTPUT_DIR/environment/container-start.stderr"; then
  container_started=true
else
  printf 'Could not start the expected perf DB image.\n' > "$CAPTURE_OUTPUT_DIR/environment/container-start.error.txt"
  exit 1
fi

ready=false
for _ in {1..120}; do
  if docker exec "$DB_CONTAINER" pg_isready --username=t_rader --dbname=t_rader > "$CAPTURE_OUTPUT_DIR/environment/pg-isready.stdout" 2> "$CAPTURE_OUTPUT_DIR/environment/pg-isready.stderr"; then
    ready=true
    break
  fi
  sleep 1
done
if [[ "$ready" != true ]]; then
  docker logs "$DB_CONTAINER" > "$CAPTURE_OUTPUT_DIR/environment/db-startup.log" 2>&1 || true
  printf 'Perf DB did not become ready within 120 seconds.\n' > "$CAPTURE_OUTPUT_DIR/environment/db-startup.error.txt"
  exit 1
fi

docker inspect --format 'memory_bytes={{.HostConfig.Memory}} memory_swap_bytes={{.HostConfig.MemorySwap}} command={{json .Config.Cmd}}' "$DB_CONTAINER" > "$CAPTURE_OUTPUT_DIR/environment/container-runtime.txt"
docker stats --no-stream --format 'cpu={{.CPUPerc}} memory={{.MemUsage}} memory_percent={{.MemPerc}}' "$DB_CONTAINER" > "$CAPTURE_OUTPUT_DIR/environment/container-stats.txt"

profile_status=0
if docker exec -i "$DB_CONTAINER" psql \
  -X -q -U t_rader -d t_rader \
  -v ON_ERROR_STOP=1 -v record_profile=false \
  < backend/perf-db/verify-profile.sql \
  > "$CAPTURE_OUTPUT_DIR/profile/verify-profile.stdout" \
  2> "$CAPTURE_OUTPUT_DIR/profile/verify-profile.stderr"; then
  profile_status=0
else
  profile_status=$?
fi
printf '%s\n' "$profile_status" > "$CAPTURE_OUTPUT_DIR/profile/verify-profile.exit_code"
if ((profile_status != 0)); then
  printf 'The saved synthetic profile did not pass verification; replay and plans were skipped.\n' > "$CAPTURE_OUTPUT_DIR/profile/verify-profile.result.txt"
  exit 1
fi

docker exec "$DB_CONTAINER" psql -X -qAt -U t_rader -d t_rader \
  -c "SELECT jsonb_pretty(to_jsonb(validation)) FROM query_perf_profile_validation AS validation WHERE profile_id = 'synthetic-market-v1'" \
  > "$CAPTURE_OUTPUT_DIR/profile/saved-profile.json" \
  2> "$CAPTURE_OUTPUT_DIR/profile/saved-profile.stderr"
docker exec "$DB_CONTAINER" psql -X -qAt -U t_rader -d t_rader \
  -c "SELECT reference_date::text FROM query_perf_seed_metadata WHERE profile_id = 'synthetic-market-v1'" \
  > "$CAPTURE_OUTPUT_DIR/profile/reference-date.txt" \
  2> "$CAPTURE_OUTPUT_DIR/profile/reference-date.stderr"
docker exec "$DB_CONTAINER" psql -X -qAt -U t_rader -d t_rader \
  -c "SELECT max(timestamp)::date::text FROM bars WHERE timeframe = '1d'" \
  > "$CAPTURE_OUTPUT_DIR/profile/replay-target-date.txt" \
  2> "$CAPTURE_OUTPUT_DIR/profile/replay-target-date.stderr"

reference_date=$(< "$CAPTURE_OUTPUT_DIR/profile/reference-date.txt")
target_date=$(< "$CAPTURE_OUTPUT_DIR/profile/replay-target-date.txt")
if [[ ! "$reference_date" =~ ^[0-9]{4}-[0-9]{2}-[0-9]{2}$ || ! "$target_date" =~ ^[0-9]{4}-[0-9]{2}-[0-9]{2}$ ]]; then
  printf 'Could not read the profile reference_date and replay target_date.\n' > "$CAPTURE_OUTPUT_DIR/profile/date-error.txt"
  exit 1
fi
if [[ "$target_date" != "$EXPECTED_REPLAY_TARGET_DATE" ]]; then
  printf 'Expected profile replay target date %s, found %s.\n' \
    "$EXPECTED_REPLAY_TARGET_DATE" "$target_date" > "$CAPTURE_OUTPUT_DIR/profile/target-date-mismatch.txt"
  exit 1
fi
reference_date_epoch=$(date -u -d "$reference_date 00:00:00" +%s)
target_date_epoch=$(date -u -d "$target_date 00:00:00" +%s)
reference_to_target_days=$(((target_date_epoch - reference_date_epoch) / 86400))
printf 'reference_date=%s\nreplay_target_date=%s\nreference_to_target_days=%s\ntarget_date_query=max(timestamp)::date FROM bars WHERE timeframe = '\''1d'\''\n' \
  "$reference_date" "$target_date" "$reference_to_target_days" > "$CAPTURE_OUTPUT_DIR/profile/dates.txt"

docker exec "$DB_CONTAINER" psql -X -q -U t_rader -d t_rader \
  -c "SELECT name, setting, unit, source FROM pg_settings WHERE name IN ('server_version', 'shared_buffers', 'work_mem', 'max_parallel_workers', 'max_parallel_workers_per_gather', 'plan_cache_mode', 'statement_timeout', 'TimeZone', 'effective_cache_size', 'temp_buffers', 'jit', 'random_page_cost', 'effective_io_concurrency') OR name LIKE 'timescaledb.%' ORDER BY name" \
  > "$CAPTURE_OUTPUT_DIR/environment/planner-settings.txt" \
  2> "$CAPTURE_OUTPUT_DIR/environment/planner-settings.stderr"
docker exec "$DB_CONTAINER" psql -X -q -U t_rader -d t_rader \
  -c 'SELECT version(); SELECT extversion FROM pg_extension WHERE extname = '\''timescaledb'\''' \
  > "$CAPTURE_OUTPUT_DIR/environment/database-version.txt" \
  2> "$CAPTURE_OUTPUT_DIR/environment/database-version.stderr"
docker exec "$DB_CONTAINER" psql -X -qAt -U t_rader -d t_rader \
  -c "BEGIN READ ONLY; SET LOCAL statement_timeout = '5s'; SHOW statement_timeout; ROLLBACK" \
  > "$CAPTURE_OUTPUT_DIR/environment/capture-statement-timeout.txt" \
  2> "$CAPTURE_OUTPUT_DIR/environment/capture-statement-timeout.stderr"

replay_started=$(date +%s%N)
replay_status=0
if DATABASE_URL="$DB_URL" cargo run --manifest-path backend/Cargo.toml -p query-perf -- --log-file "$QUERY_LOG_FILE" \
  > "$CAPTURE_OUTPUT_DIR/replay/stdout.txt" \
  2> "$CAPTURE_OUTPUT_DIR/replay/stderr.txt"; then
  replay_status=0
else
  replay_status=$?
  overall_status=1
fi
replay_elapsed_ms=$((($(date +%s%N) - replay_started) / 1000000))
if grep -Eq 'FAIL statement timeout|query-perf found [1-9][0-9]* failing SQL statement' \
  "$CAPTURE_OUTPUT_DIR/replay/stdout.txt" "$CAPTURE_OUTPUT_DIR/replay/stderr.txt"; then
  overall_status=1
fi
printf '%s\n' "$replay_status" > "$CAPTURE_OUTPUT_DIR/replay/exit_code"
printf '%s\n' "$replay_elapsed_ms" > "$CAPTURE_OUTPUT_DIR/replay/elapsed_ms"
printf 'exit_code=%s\nelapsed_ms=%s\n' "$replay_status" "$replay_elapsed_ms" > "$CAPTURE_OUTPUT_DIR/replay/result.txt"

if ! grep -Eq "^Replayed ${EXPECTED_REPLAY_SQL_COUNT} SQL statement\\(s\\) against data through ${EXPECTED_REPLAY_TARGET_DATE}\\.$" \
  "$CAPTURE_OUTPUT_DIR/replay/stdout.txt"; then
  printf 'expected_replay_sql_count=%s\nexpected_target_date=%s\n' \
    "$EXPECTED_REPLAY_SQL_COUNT" "$EXPECTED_REPLAY_TARGET_DATE" \
    > "$CAPTURE_OUTPUT_DIR/replay/replay-input-validation-error.txt"
  printf 'Replay summary did not match the fixed SQL count and target date; EXPLAIN was skipped.\n' \
    >> "$CAPTURE_OUTPUT_DIR/replay/replay-input-validation-error.txt"
  exit 1
fi

applied_exclusions_file="$CAPTURE_OUTPUT_DIR/replay/applied-exclusions.json"
if cargo run --locked --manifest-path backend/scripts/query-perf-input-manifest/Cargo.toml -- \
  verify-replay-exclusions \
  --stdout "$CAPTURE_OUTPUT_DIR/replay/stdout.txt" \
  --exclusions-file backend/crates/apps/query-perf/exclusions.json \
  --output "$applied_exclusions_file" \
  > "$CAPTURE_OUTPUT_DIR/replay/exclusion-validation.stdout" \
  2> "$CAPTURE_OUTPUT_DIR/replay/exclusion-validation.stderr"; then
  expected_exclusion_sha256=$(jq -r '.provenance.reference_applied_exclusion_set_sha256' "$INPUT_MANIFEST_FILE")
  if ! jq -e \
    --arg expected_sha256 "$expected_exclusion_sha256" \
    --argjson expected_count "$EXPECTED_APPLIED_EXCLUSION_COUNT" \
    '.matches_configured_set == true and .configured_count == $expected_count and .observed_count == $expected_count and .missing_count == 0 and .unexpected_count == 0 and .configured_set_sha256 == $expected_sha256' \
    "$applied_exclusions_file" \
    > "$CAPTURE_OUTPUT_DIR/replay/exclusion-reference-validation.stdout" \
    2> "$CAPTURE_OUTPUT_DIR/replay/exclusion-reference-validation.stderr"; then
    printf 'Applied exclusion set differs from the original replay reference.\n' \
      > "$CAPTURE_OUTPUT_DIR/replay/applied-exclusion-mismatch.txt"
    exit 1
  fi
else
  printf 'Could not verify the applied exclusion set; EXPLAIN was skipped.\n' \
    > "$CAPTURE_OUTPUT_DIR/replay/applied-exclusion-validation-error.txt"
  exit 1
fi

if jq -s -e '
  [ .[] | select((.message // "") | contains("eligible_stocks AS") and contains("period_bars AS MATERIALIZED")) ] as $rows
  | if ($rows | length) != 6 then error("expected 6 LIST_GROUP_MOVERS_SQL log rows")
    elif ([$rows[] | .message | sub("^execute [^:]+: "; "")] | unique | length) != 1 then error("LIST_GROUP_MOVERS_SQL differs between log rows")
    else true end
' "$QUERY_LOG_FILE" > "$CAPTURE_OUTPUT_DIR/sql/source-query-validation.stdout" 2> "$CAPTURE_OUTPUT_DIR/sql/source-query-validation.stderr"; then
  :
else
  printf 'Could not uniquely identify the six LIST_GROUP_MOVERS_SQL log rows.\n' > "$CAPTURE_OUTPUT_DIR/sql/source-query-validation.error.txt"
  exit 1
fi

for required_source in \
  'static LIST_GROUP_MOVERS_SQL' \
  'eligible_stocks AS (' \
  'period_bars AS MATERIALIZED (' \
  'latest_period_bars AS (' \
  'avg_turnover AS MATERIALIZED (' \
  'ranked_returns AS ('; do
  if ! grep -Fq -- "$required_source" "$SOURCE_SQL_PATH"; then
    printf 'Source query does not contain expected SQL fragment: %s\n' "$required_source" >> "$CAPTURE_OUTPUT_DIR/sql/source-query-validation.error.txt"
    exit 1
  fi
done

jq -s -er '
  [ .[] | select((.message // "") | contains("eligible_stocks AS") and contains("period_bars AS MATERIALIZED")) | .message | sub("^execute [^:]+: "; "") ]
  | unique
  | if length == 1 then .[0] else error("source SQL was not unique") end
' "$QUERY_LOG_FILE" > "$CAPTURE_OUTPUT_DIR/sql/full.sql"

if ! jq -e 'length == 6 and all(.[]; .parameter_types == ["text", "date", "date", "numeric", "bigint"] and (.original_parameters | length) == 5 and (.shifted_parameters | length) == 5)' \
  "$INPUT_GROUP_BINDS_FILE" > "$CAPTURE_OUTPUT_DIR/binds/source-validation.stdout" \
  2> "$CAPTURE_OUTPUT_DIR/binds/source-validation.stderr"; then
  printf 'Expected six source-shifted LIST_GROUP_MOVERS_SQL binds with application parameter types.\n' \
    > "$CAPTURE_OUTPUT_DIR/binds/validation-error.txt"
  exit 1
fi

perl -0777 -ne 's/SELECT group_metrics\.group_key,.*\z//s or die "could not locate LIST_GROUP_MOVERS_SQL final SELECT\n"; print' \
  "$CAPTURE_OUTPUT_DIR/sql/full.sql" > "$CAPTURE_OUTPUT_DIR/sql/cte-prefix.sql"
if ! grep -Eq '^WITH requested_axes AS' "$CAPTURE_OUTPUT_DIR/sql/cte-prefix.sql"; then
  printf 'Extracted SQL prefix does not begin with the application CTE chain.\n' > "$CAPTURE_OUTPUT_DIR/sql/component-error.txt"
  exit 1
fi

for component in period_bars latest_period_bars stock_change_prices avg_turnover group_returns ranked_returns; do
  {
    cat "$CAPTURE_OUTPUT_DIR/sql/cte-prefix.sql"
    printf 'SELECT * FROM %s\n' "$component"
  } > "$CAPTURE_OUTPUT_DIR/sql/$component.sql"
done

if ! jq -e '.comparison.group_movers.bind_count == 6 and .comparison.group_movers.specified_bind_count == 4 and (.comparison.group_movers.specified_bind_hashes | length) == 4' \
  "$INPUT_MANIFEST_FILE" > "$CAPTURE_OUTPUT_DIR/binds/source-manifest-validation.stdout" \
  2> "$CAPTURE_OUTPUT_DIR/binds/source-manifest-validation.stderr"; then
  printf 'The source bind hash set does not contain the expected six and four bind counts.\n' \
    > "$CAPTURE_OUTPUT_DIR/binds/validation-error.txt"
  exit 1
fi

bind_index=0
while IFS= read -r bind_json; do
  bind_index=$((bind_index + 1))
  bind_id=$(printf 'bind-%02d' "$bind_index")
  original_parameters=$(jq -c '.original_parameters' <<< "$bind_json")
  shifted_parameter_values=$(jq -c '.shifted_parameters' <<< "$bind_json")
  parameter_types=$(jq -c '.parameter_types' <<< "$bind_json")
  axis=$(jq -r '.shifted_parameters[0]' <<< "$bind_json")
  from_date=$(jq -r '.shifted_parameters[1]' <<< "$bind_json")
  to_date=$(jq -r '.shifted_parameters[2]' <<< "$bind_json")
  min_turnover=$(jq -r '.shifted_parameters[3]' <<< "$bind_json")
  limit=$(jq -r '.shifted_parameters[4]' <<< "$bind_json")
  shifted_parameters="Shifted parameters: [Some(\"$axis\"), Some(\"$from_date\"), Some(\"$to_date\"), Some(\"$min_turnover\"), Some(\"$limit\")]"
  if replay_failure_kind=$(awk -v target="$shifted_parameters" '
    /^FAIL / {
      failure_kind = $0
      sub(/^FAIL /, "", failure_kind)
      sub(/ after .*$/, "", failure_kind)
    }
    index($0, target) {
      print (failure_kind == "" ? "failure kind unavailable" : failure_kind)
      found = 1
      exit
    }
    END {
      if (!found) exit 1
    }
  ' "$CAPTURE_OUTPUT_DIR/replay/stderr.txt"); then
    replay_outcome=$replay_failure_kind
  else
    replay_outcome='no failure recorded'
  fi
  jq -n \
    --arg id "$bind_id" \
    --arg replay_outcome "$replay_outcome" \
    --arg target_date "$target_date" \
    --argjson parameter_types "$parameter_types" \
    --argjson original_parameters "$original_parameters" \
    --argjson shifted_parameters "$shifted_parameter_values" \
    '{
      bind_id: $id,
      replay_outcome: $replay_outcome,
      target_date: $target_date,
      parameter_types: $parameter_types,
      original_parameters: $original_parameters,
      shifted_parameters: $shifted_parameters
    }' > "$CAPTURE_OUTPUT_DIR/binds/$bind_id.json"
done < <(jq -c '.[]' "$INPUT_GROUP_BINDS_FILE")
jq -s '.' "$CAPTURE_OUTPUT_DIR"/binds/bind-*.json > "$CAPTURE_OUTPUT_DIR/binds/binds.json"

run_explain() {
  local bind_id=$1
  local axis=$2
  local from_date=$3
  local to_date=$4
  local min_turnover=$5
  local limit=$6
  local query_file=$7
  local observation=$8
  local analyze=$9
  local plan_dir="$CAPTURE_OUTPUT_DIR/plans/$bind_id"
  local psql_file="$plan_dir/$observation.psql"
  local stdout_file="$plan_dir/$observation.stdout"
  local stderr_file="$plan_dir/$observation.stderr"
  local started
  local command_status
  local elapsed_ms
  local explain_options='FORMAT JSON'
  mkdir -p "$plan_dir"

  if [[ "$analyze" == true ]]; then
    explain_options='ANALYZE, BUFFERS, FORMAT JSON'
  fi

  {
    printf 'BEGIN READ ONLY;\n'
    printf "SET LOCAL statement_timeout = '5s';\n"
    printf 'PREPARE capture_statement(text, date, date, numeric, bigint) AS\n'
    cat "$query_file"
    printf ';\n'
    printf '\\if :ERROR\nROLLBACK;\n\\quit 1\n\\endif\n'
    printf 'EXPLAIN (%s) EXECUTE capture_statement(\047%s\047, \047%s\047, \047%s\047, %s, %s);\n' \
      "$explain_options" "$axis" "$from_date" "$to_date" "$min_turnover" "$limit"
    printf '\\if :ERROR\nROLLBACK;\n\\quit 1\n\\endif\n'
    printf 'ROLLBACK;\n'
  } > "$psql_file"

  started=$(date +%s%N)
  if docker exec -i "$DB_CONTAINER" psql -X -qAt -U t_rader -d t_rader \
    < "$psql_file" > "$stdout_file" 2> "$stderr_file"; then
    command_status=0
  else
    command_status=$?
    overall_status=1
  fi
  elapsed_ms=$((($(date +%s%N) - started) / 1000000))
  printf '%s\n' "$command_status" > "$plan_dir/$observation.exit_code"
  printf '%s\n' "$elapsed_ms" > "$plan_dir/$observation.elapsed_ms"
  if [[ "$analyze" != true && "$command_status" == 0 ]]; then
    cp "$stdout_file" "$plan_dir/$observation.plan.json"
  elif [[ "$analyze" == true && "$command_status" == 0 ]]; then
    cp "$stdout_file" "$plan_dir/$observation.plan.json"
  fi
  printf '%s\t%s\t%s\t%s\n' "$bind_id" "$observation" "$command_status" "$elapsed_ms" >> "$CAPTURE_OUTPUT_DIR/observation-order.tsv"
}

cat > "$CAPTURE_OUTPUT_DIR/README.md" << 'EOF'
# Query plan capture

- 入力は固定 source の backend test を Actions 内で再実行して生成した `regenerated input` です。消失した元 artifact と byte 同一とは主張しません。元 ZIP の SHA-256 は upload log から記録した参照値で、手元に ZIP がなく同一性は検証できていません。
- `input/` には raw SQL/bind を含めない manifest と全件比較結果を保存しています。再生成 JSONL は別の `backend-query-log-regenerated-*` artifact として retention 1 日で保存します。
- `binds/source-binds.json` には regenerated input の対象 6 bind と source `shift_parameters` の出力を保存します。shell 側では parameter 値を変換しません。
- `replay/` には regenerated input の 350 SQL replay の stdout、stderr、exit code と適用除外集合の照合結果を保存しています。EXPLAIN の観測結果とは分けています。
- `sql/full.sql` は source query log から抽出した application SQL です。component SQL は元の CTE 全体を保持し、終端 SELECT だけを `SELECT * FROM <component>` に置き換えています。
- 各 EXPLAIN は新しい PostgreSQL connection と read-only transaction で実行し、`statement_timeout = '5s'` を設定します。application と同じ parameter type を使い、bind ごとに新しい prepared statement の初回実行として計画します。error と timeout は個別の stdout、stderr、exit code、所要時間を記録します。
- profile の `reference_date` と replay の `target_date` は別々に記録しています。EXPLAIN は replay の後に実行するため buffer cache が warm で、`EXPLAIN ANALYZE` には計測 overhead もあります。full query と component の所要時間は別観測なので、合算しないでください。
EOF

printf 'bind_id\tobservation\texit_code\telapsed_ms\n' > "$CAPTURE_OUTPUT_DIR/observation-order.tsv"
for bind_json in "$CAPTURE_OUTPUT_DIR"/binds/bind-*.json; do
  bind_id=$(jq -r '.bind_id' "$bind_json")
  axis=$(jq -r '.shifted_parameters[0]' "$bind_json")
  from_date=$(jq -r '.shifted_parameters[1]' "$bind_json")
  to_date=$(jq -r '.shifted_parameters[2]' "$bind_json")
  min_turnover=$(jq -r '.shifted_parameters[3]' "$bind_json")
  limit=$(jq -r '.shifted_parameters[4]' "$bind_json")
  run_explain "$bind_id" "$axis" "$from_date" "$to_date" "$min_turnover" "$limit" "$CAPTURE_OUTPUT_DIR/sql/full.sql" 'full.explain' false
  run_explain "$bind_id" "$axis" "$from_date" "$to_date" "$min_turnover" "$limit" "$CAPTURE_OUTPUT_DIR/sql/full.sql" 'full.analyze' true
  if [[ "$(cat "$CAPTURE_OUTPUT_DIR/plans/$bind_id/full.explain.exit_code")" != 0 || "$(cat "$CAPTURE_OUTPUT_DIR/plans/$bind_id/full.analyze.exit_code")" != 0 ]]; then
    for component in period_bars latest_period_bars stock_change_prices avg_turnover group_returns ranked_returns; do
      run_explain "$bind_id" "$axis" "$from_date" "$to_date" "$min_turnover" "$limit" "$CAPTURE_OUTPUT_DIR/sql/$component.sql" "$component.explain" false
      run_explain "$bind_id" "$axis" "$from_date" "$to_date" "$min_turnover" "$limit" "$CAPTURE_OUTPUT_DIR/sql/$component.sql" "$component.analyze" true
    done
  fi
done

plans_collected=true
exit "$overall_status"
