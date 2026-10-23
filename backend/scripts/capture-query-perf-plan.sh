#!/usr/bin/env bash
set -euo pipefail

readonly EXPECTED_IMAGE_REF='ghcr.io/fohte/t-rader/perf-db:e8715ae2cb74fa0a8a05d65f7af4ccfea9cb7658c56cb9bd2ee604572719922d'
readonly SOURCE_RUN_ID='39714934810'
readonly SOURCE_ARTIFACT_ID='12399056019'
readonly SOURCE_PR_HEAD_SHA='556c691d809b11afa566533aa98f6a02bf68ef69'
readonly SOURCE_CHECKOUT_SHA='d24a3e71dac02684f1d752c4d37aeb6462ca5c91'
readonly DB_CONTAINER='query-perf-plan-capture-db'
readonly DB_URL='postgres://t_rader:t_rader@127.0.0.1:5432/t_rader'
readonly QUERY_LOG_FILE="${QUERY_LOG_FILE:-backend-query-log/postgresql.json}"
readonly CAPTURE_OUTPUT_DIR="${CAPTURE_OUTPUT_DIR:-query-perf-plan-capture}"
readonly SOURCE_SQL_PATH='backend/crates/gateways/postgres/src/group_movers.rs'

overall_status=0
container_started=false
mkdir -p "$CAPTURE_OUTPUT_DIR"/{environment,profile,replay,sql,binds,plans}

# EXIT trap から呼ぶため、直接の関数呼び出しとしては検出されない。
# shellcheck disable=SC2329
cleanup() {
  if [[ "$container_started" == true ]]; then
    docker rm --force "$DB_CONTAINER" > "$CAPTURE_OUTPUT_DIR/environment/container-cleanup.stdout" 2> "$CAPTURE_OUTPUT_DIR/environment/container-cleanup.stderr" || true
  fi
}
trap cleanup EXIT

record_environment() {
  {
    printf 'diagnostic_checkout_sha=%s\n' "$(git rev-parse HEAD)"
    printf 'diagnostic_branch=%s\n' "$(git branch --show-current)"
    printf 'source_checkout_sha=%s\n' "$SOURCE_CHECKOUT_SHA"
    printf 'source_pr_head_sha=%s\n' "$SOURCE_PR_HEAD_SHA"
    printf 'source_run_url=https://github.com/fohte/t-rader/actions/runs/%s\n' "$SOURCE_RUN_ID"
    printf 'source_run_id=%s\n' "$SOURCE_RUN_ID"
    printf 'source_artifact_id=%s\n' "$SOURCE_ARTIFACT_ID"
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

if [[ ! -f "$QUERY_LOG_FILE" ]]; then
  printf 'Missing source query log: %s\n' "$QUERY_LOG_FILE" > "$CAPTURE_OUTPUT_DIR/replay/source-log-error.txt"
  exit 1
fi

if [[ "$(git rev-parse HEAD^)" != "$SOURCE_CHECKOUT_SHA" ]]; then
  printf 'Expected diagnostic commit parent %s, found %s\n' "$SOURCE_CHECKOUT_SHA" "$(git rev-parse HEAD^)" > "$CAPTURE_OUTPUT_DIR/environment/source-checkout-error.txt"
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
  if rg -q 'no such manifest|manifest unknown|404 Not Found' \
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
if rg -q 'FAIL statement timeout|query-perf found [1-9][0-9]* failing SQL statement' \
  "$CAPTURE_OUTPUT_DIR/replay/stdout.txt" "$CAPTURE_OUTPUT_DIR/replay/stderr.txt"; then
  overall_status=1
fi
printf '%s\n' "$replay_status" > "$CAPTURE_OUTPUT_DIR/replay/exit_code"
printf '%s\n' "$replay_elapsed_ms" > "$CAPTURE_OUTPUT_DIR/replay/elapsed_ms"
printf 'exit_code=%s\nelapsed_ms=%s\n' "$replay_status" "$replay_elapsed_ms" > "$CAPTURE_OUTPUT_DIR/replay/result.txt"

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
  if ! rg -Fq "$required_source" "$SOURCE_SQL_PATH"; then
    printf 'Source query does not contain expected SQL fragment: %s\n' "$required_source" >> "$CAPTURE_OUTPUT_DIR/sql/source-query-validation.error.txt"
    exit 1
  fi
done

jq -s -er '
  [ .[] | select((.message // "") | contains("eligible_stocks AS") and contains("period_bars AS MATERIALIZED")) | .message | sub("^execute [^:]+: "; "") ]
  | unique
  | if length == 1 then .[0] else error("source SQL was not unique") end
' "$QUERY_LOG_FILE" > "$CAPTURE_OUTPUT_DIR/sql/full.sql"

jq -r '
  select((.message // "") | contains("eligible_stocks AS") and contains("period_bars AS MATERIALIZED"))
  | .detail
  | capture("Parameters: \\$1 = \u0027(?<p1>[^\u0027]*)\u0027, \\$2 = \u0027(?<p2>[^\u0027]*)\u0027, \\$3 = \u0027(?<p3>[^\u0027]*)\u0027, \\$4 = \u0027(?<p4>[^\u0027]*)\u0027, \\$5 = \u0027(?<p5>[^\u0027]*)\u0027")
  | [.p1, .p2, .p3, .p4, .p5]
  | @tsv
' "$QUERY_LOG_FILE" | LC_ALL=C sort -u > "$CAPTURE_OUTPUT_DIR/binds/original.tsv"

if [[ "$(wc -l < "$CAPTURE_OUTPUT_DIR/binds/original.tsv" | tr -d ' ')" != 6 ]]; then
  printf 'Expected six unique parameter sets for LIST_GROUP_MOVERS_SQL.\n' > "$CAPTURE_OUTPUT_DIR/binds/validation-error.txt"
  exit 1
fi

perl -0777 -ne 's/SELECT group_metrics\.group_key,.*\z//s or die "could not locate LIST_GROUP_MOVERS_SQL final SELECT\n"; print' \
  "$CAPTURE_OUTPUT_DIR/sql/full.sql" > "$CAPTURE_OUTPUT_DIR/sql/cte-prefix.sql"
if ! rg -q '^WITH requested_axes AS' "$CAPTURE_OUTPUT_DIR/sql/cte-prefix.sql"; then
  printf 'Extracted SQL prefix does not begin with the application CTE chain.\n' > "$CAPTURE_OUTPUT_DIR/sql/component-error.txt"
  exit 1
fi

for component in period_bars latest_period_bars stock_change_prices avg_turnover group_returns ranked_returns; do
  {
    cat "$CAPTURE_OUTPUT_DIR/sql/cte-prefix.sql"
    printf 'SELECT * FROM %s\n' "$component"
  } > "$CAPTURE_OUTPUT_DIR/sql/$component.sql"
done

shifted_tsv="$CAPTURE_OUTPUT_DIR/binds/shifted.tsv"
: > "$shifted_tsv"
while IFS=$'\t' read -r axis original_from original_to min_turnover limit; do
  if [[ ! "$axis" =~ ^[A-Za-z0-9-]+$ || ! "$original_from" =~ ^[0-9]{4}-[0-9]{2}-[0-9]{2}$ || ! "$original_to" =~ ^[0-9]{4}-[0-9]{2}-[0-9]{2}$ || ! "$min_turnover" =~ ^[0-9]+$ || ! "$limit" =~ ^[0-9]+$ ]]; then
    printf 'Unexpected source bind value format.\n' > "$CAPTURE_OUTPUT_DIR/binds/validation-error.txt"
    exit 1
  fi

  latest_date=$original_from
  if [[ "$original_to" > "$latest_date" ]]; then
    latest_date=$original_to
  fi
  target_epoch=$(date -u -d "$target_date 00:00:00" +%s)
  latest_epoch=$(date -u -d "$latest_date 00:00:00" +%s)
  day_shift=$(((target_epoch - latest_epoch) / 86400))
  from_epoch=$(date -u -d "$original_from 00:00:00" +%s)
  to_epoch=$(date -u -d "$original_to 00:00:00" +%s)
  shifted_from=$(date -u -d "@$((from_epoch + day_shift * 86400))" +%F)
  shifted_to=$(date -u -d "@$((to_epoch + day_shift * 86400))" +%F)
  printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n' \
    "$axis" "$original_from" "$original_to" "$shifted_from" "$shifted_to" "$min_turnover" "$limit" "$day_shift" \
    >> "$shifted_tsv"
done < "$CAPTURE_OUTPUT_DIR/binds/original.tsv"

expected_shifted_binds=$(
  cat << 'EXPECTED'
demo-axis	2026-10-18	2026-10-22	0	5
sample-axis	2026-10-21	2026-10-22	0	5
sample-axis	2026-10-21	2026-10-22	150	5
sample-axis	2026-10-22	2026-10-22	0	5
EXPECTED
)
actual_shifted_binds=$(cut -f1,4,5,6,7 "$shifted_tsv" | LC_ALL=C sort -u)
if [[ "$(printf '%s\n' "$actual_shifted_binds" | LC_ALL=C sort -u)" != "$(printf '%s\n' "$expected_shifted_binds" | LC_ALL=C sort -u)" ]]; then
  printf 'Expected and extracted shifted parameter sets differ.\n' > "$CAPTURE_OUTPUT_DIR/binds/validation-error.txt"
  printf 'expected:\n%s\nactual:\n%s\n' "$expected_shifted_binds" "$actual_shifted_binds" >> "$CAPTURE_OUTPUT_DIR/binds/validation-error.txt"
  exit 1
fi

bind_index=0
while IFS=$'\t' read -r axis original_from original_to shifted_from shifted_to min_turnover limit day_shift; do
  bind_index=$((bind_index + 1))
  bind_id=$(printf 'bind-%02d' "$bind_index")
  shifted_parameters="Shifted parameters: [Some(\"$axis\"), Some(\"$shifted_from\"), Some(\"$shifted_to\"), Some(\"$min_turnover\"), Some(\"$limit\")]"
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
    --argjson day_shift "$day_shift" \
    --arg original_axis "$axis" \
    --arg original_from "$original_from" \
    --arg original_to "$original_to" \
    --arg shifted_from "$shifted_from" \
    --arg shifted_to "$shifted_to" \
    --arg min_turnover "$min_turnover" \
    --arg limit "$limit" \
    ' {
      bind_id: $id,
      replay_outcome: $replay_outcome,
      target_date: $target_date,
      day_shift: $day_shift,
      parameter_types: ["text", "date", "date", "numeric", "bigint"],
      original_parameters: [$original_axis, $original_from, $original_to, $min_turnover, $limit],
      shifted_parameters: [$original_axis, $shifted_from, $shifted_to, $min_turnover, $limit]
    }' > "$CAPTURE_OUTPUT_DIR/binds/$bind_id.json"
done < "$shifted_tsv"
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

- `replay/` には source artifact の 350 SQL replay の stdout、stderr、exit code を保存しています。EXPLAIN の観測結果とは分けています。
- `sql/full.sql` は source query log から抽出した application SQL です。component SQL は元の CTE 全体を保持し、終端 SELECT だけを `SELECT * FROM <component>` に置き換えています。
- 各 EXPLAIN は新しい PostgreSQL connection と read-only transaction で実行し、`statement_timeout = '5s'` を設定します。application と同じ parameter type を使い、bind ごとに新しい prepared statement の初回実行として計画します。error と timeout は個別の stdout、stderr、exit code、所要時間を記録します。
- profile の `reference_date` と replay の `target_date` は別々に記録しています。EXPLAIN は元 replay の後に実行するため buffer cache が warm で、`EXPLAIN ANALYZE` には計測 overhead もあります。full query と component の所要時間は別観測なので、合算しないでください。
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

printf 'replay_exit_code=%s\noverall_status=%s\n' "$replay_status" "$overall_status" > "$CAPTURE_OUTPUT_DIR/result.txt"
exit "$overall_status"
