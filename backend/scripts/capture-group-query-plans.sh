#!/usr/bin/env bash
set -euo pipefail

readonly EXPECTED_IMAGE_REF='ghcr.io/fohte/t-rader/perf-db:e8715ae2cb74fa0a8a05d65f7af4ccfea9cb7658c56cb9bd2ee604572719922d'
readonly EXPECTED_REPLAY_TARGET_DATE='2026-10-22'
readonly EXPECTED_GROUP_BIND_COUNT=6
readonly SOURCE_CHECKOUT_SHA='d24a3e71dac02684f1d752c4d37aeb6462ca5c91'
readonly SOURCE_RUN_ID='39714934810'
readonly SOURCE_ARTIFACT_ID='12399056019'
readonly SOURCE_LOCAL_LOG_SHA256='6a095a31d0d22eaf1517c23ae48b374056017ad7eabf26fd882ebf6b6af3320b'
readonly SOURCE_ZIP_SHA256='50d7b38e3a07a9939cc87f9e1bdbf5084240c9ab13ecd53e1ddb34ad50b499e6'
readonly SOURCE_PR_HEAD_SHA='556c691d809b11afa566533aa98f6a02bf68ef69'
readonly DB_CONTAINER='query-perf-group-plan-capture-db'
readonly DB_NAME='t_rader'
readonly DB_USER='t_rader'
readonly QUERY_LOG_FILE="${QUERY_LOG_FILE:-$RUNNER_TEMP/backend-query-log-regenerated/postgresql.json}"
readonly INPUT_ARTIFACT_NAME="${INPUT_ARTIFACT_NAME:-}"
readonly INPUT_ARTIFACT_ID="${INPUT_ARTIFACT_ID:-}"
readonly INPUT_MANIFEST_FILE="${INPUT_MANIFEST_FILE:-query-perf-plan-capture/input/regenerated-manifest.json}"
readonly INPUT_COMPARISON_FILE="${INPUT_COMPARISON_FILE:-query-perf-plan-capture/input/comparison.json}"
readonly INPUT_GROUP_BINDS_FILE="${INPUT_GROUP_BINDS_FILE:-query-perf-plan-capture/binds/source-binds.json}"
readonly CAPTURE_OUTPUT_DIR="${CAPTURE_OUTPUT_DIR:-query-perf-plan-capture}"

overall_status=0
container_started=false
plans_recorded=false
binds_recorded=0
mkdir -p "$CAPTURE_OUTPUT_DIR"/{environment,profile,input,replay,sql,binds,plans}

# shellcheck disable=SC2329
cleanup() {
  local command_status=$?
  trap - EXIT
  if [[ "$container_started" == true ]]; then
    docker rm --force "$DB_CONTAINER" \
      > "$CAPTURE_OUTPUT_DIR/environment/container-cleanup.stdout" \
      2> "$CAPTURE_OUTPUT_DIR/environment/container-cleanup.stderr" || true
  fi
  if ((command_status == 0 && overall_status != 0)); then
    command_status=$overall_status
  fi
  local result_status='failed'
  if ((command_status == 0)) && [[ "$plans_recorded" == true ]]; then
    result_status='observed'
  fi
  jq -n \
    --arg status "$result_status" \
    --argjson script_exit_code "$command_status" \
    --argjson binds_recorded "$binds_recorded" \
    --argjson plans_recorded "$plans_recorded" \
    '{status: $status, script_exit_code: $script_exit_code, replay_performed: false, binds_recorded: $binds_recorded, plans_recorded: $plans_recorded}' \
    > "$CAPTURE_OUTPUT_DIR/result.json"
  printf 'status=%s\nscript_exit_code=%s\nreplay_performed=false\nbinds_recorded=%s\nplans_recorded=%s\n' \
    "$result_status" "$command_status" "$binds_recorded" "$plans_recorded" \
    > "$CAPTURE_OUTPUT_DIR/result.txt"
  exit "$command_status"
}
trap cleanup EXIT

record_environment() {
  local log_sha256='unavailable'
  local manifest_sha256='unavailable'
  local comparison_matches='unavailable'
  local group_matches='unavailable'
  local binds_sha256='unavailable'
  if [[ -f "$QUERY_LOG_FILE" ]]; then
    log_sha256=$(sha256sum "$QUERY_LOG_FILE" | cut -d ' ' -f 1)
  fi
  if [[ -f "$INPUT_MANIFEST_FILE" ]]; then
    manifest_sha256=$(sha256sum "$INPUT_MANIFEST_FILE" | cut -d ' ' -f 1)
  fi
  if [[ -f "$INPUT_COMPARISON_FILE" ]]; then
    comparison_matches=$(jq -r '.matches // false' "$INPUT_COMPARISON_FILE")
    group_matches=$(jq -r '.group_movers_match // false' "$INPUT_COMPARISON_FILE")
  fi
  if [[ -f "$INPUT_GROUP_BINDS_FILE" ]]; then
    binds_sha256=$(sha256sum "$INPUT_GROUP_BINDS_FILE" | cut -d ' ' -f 1)
  fi
  {
    printf 'diagnostic_checkout_sha=%s\n' "$(git rev-parse HEAD)"
    printf 'diagnostic_branch=%s\n' "$(git branch --show-current)"
    printf 'diagnostic_run_url=%s/%s/actions/runs/%s\n' \
      "$GITHUB_SERVER_URL" "$GITHUB_REPOSITORY" "$GITHUB_RUN_ID"
    printf 'diagnostic_job_id=%s\n' "$GITHUB_JOB"
    printf 'source_checkout_sha=%s\n' "$SOURCE_CHECKOUT_SHA"
    printf 'source_pr_head_sha=%s\n' "$SOURCE_PR_HEAD_SHA"
    printf 'source_run_url=https://github.com/fohte/t-rader/actions/runs/%s\n' "$SOURCE_RUN_ID"
    printf 'reference_source_run_id=%s\n' "$SOURCE_RUN_ID"
    printf 'reference_source_artifact_id=%s\n' "$SOURCE_ARTIFACT_ID"
    printf 'reference_local_log_sha256=%s\n' "$SOURCE_LOCAL_LOG_SHA256"
    printf 'reference_source_zip_sha256=%s\n' "$SOURCE_ZIP_SHA256"
    printf 'reference_source_zip_identity_proven=false\n'
    printf 'input_kind=ci-regenerated\n'
    printf 'input_artifact_name=%s\n' "$INPUT_ARTIFACT_NAME"
    printf 'input_artifact_id=%s\n' "$INPUT_ARTIFACT_ID"
    printf 'input_log_sha256=%s\n' "$log_sha256"
    printf 'input_manifest_sha256=%s\n' "$manifest_sha256"
    printf 'input_group_binds_sha256=%s\n' "$binds_sha256"
    printf 'full_input_comparison_matches=%s\n' "$comparison_matches"
    printf 'group_query_comparison_matches=%s\n' "$group_matches"
    printf 'replay_performed=false\n'
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
  printf 'Diagnostic source ancestry or protected-source check failed.\n' \
    > "$CAPTURE_OUTPUT_DIR/environment/source-check-error.txt"
  exit 1
fi

if ! backend/scripts/prepare-group-query-plan-input.sh \
  "$QUERY_LOG_FILE" "$INPUT_ARTIFACT_NAME" "$INPUT_ARTIFACT_ID" \
  "$INPUT_MANIFEST_FILE" "$INPUT_COMPARISON_FILE" \
  "$INPUT_GROUP_BINDS_FILE" "$CAPTURE_OUTPUT_DIR"; then
  exit 1
fi

printf 'bind_id\tobservation\tcomponent\texit_code\telapsed_ms\tsqlstate\toutcome\n' \
  > "$CAPTURE_OUTPUT_DIR/observation-order.tsv"

computed_image_ref=''
if computed_image_ref=$(backend/scripts/build-perf-db-image.sh --image-ref \
  2> "$CAPTURE_OUTPUT_DIR/environment/image-ref.stderr"); then
  printf 'expected=%s\ncomputed=%s\n' "$EXPECTED_IMAGE_REF" "$computed_image_ref" \
    > "$CAPTURE_OUTPUT_DIR/environment/image-ref-check.txt"
else
  printf 'Could not compute the fixed perf DB image reference.\n' \
    > "$CAPTURE_OUTPUT_DIR/environment/image-ref-error.txt"
  exit 1
fi
if [[ "$computed_image_ref" != "$EXPECTED_IMAGE_REF" ]]; then
  printf 'Perf DB image reference did not match the fixed image.\n' \
    > "$CAPTURE_OUTPUT_DIR/environment/image-ref-error.txt"
  exit 1
fi

if docker manifest inspect --verbose "$EXPECTED_IMAGE_REF" \
  > "$CAPTURE_OUTPUT_DIR/environment/image-manifest.json" \
  2> "$CAPTURE_OUTPUT_DIR/environment/image-manifest.stderr"; then
  if docker pull "$EXPECTED_IMAGE_REF" \
    > "$CAPTURE_OUTPUT_DIR/environment/image-pull.stdout" \
    2> "$CAPTURE_OUTPUT_DIR/environment/image-pull.stderr"; then
    printf 'image_source=registry\n' > "$CAPTURE_OUTPUT_DIR/environment/image-summary.txt"
  else
    printf 'Could not pull the fixed perf DB image.\n' \
      > "$CAPTURE_OUTPUT_DIR/environment/image-error.txt"
    exit 1
  fi
else
  if grep -Eq 'no such manifest|manifest unknown|404 Not Found' \
    "$CAPTURE_OUTPUT_DIR/environment/image-manifest.stderr" \
    "$CAPTURE_OUTPUT_DIR/environment/image-manifest.json"; then
    printf 'image_source=local-build\n' > "$CAPTURE_OUTPUT_DIR/environment/image-summary.txt"
    if backend/scripts/build-perf-db-image.sh \
      > "$CAPTURE_OUTPUT_DIR/environment/image-build.stdout" \
      2> "$CAPTURE_OUTPUT_DIR/environment/image-build.stderr"; then
      :
    else
      build_status=$?
      printf 'build_exit_code=%s\n' "$build_status" \
        > "$CAPTURE_OUTPUT_DIR/environment/image-error.txt"
      exit 1
    fi
  else
    printf 'Registry inspection failed for the fixed perf DB image.\n' \
      > "$CAPTURE_OUTPUT_DIR/environment/image-error.txt"
    exit 1
  fi
fi

docker image inspect --format '{{.Id}}' "$EXPECTED_IMAGE_REF" \
  > "$CAPTURE_OUTPUT_DIR/environment/image-id.txt"
docker image inspect --format '{{json .RepoDigests}}' "$EXPECTED_IMAGE_REF" \
  > "$CAPTURE_OUTPUT_DIR/environment/image-digests.json"
docker image inspect --format '{{index .Config.Labels "org.opencontainers.image.source"}}' "$EXPECTED_IMAGE_REF" \
  > "$CAPTURE_OUTPUT_DIR/environment/image-source.txt"
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
  printf 'Could not start the fixed perf DB image.\n' \
    > "$CAPTURE_OUTPUT_DIR/environment/container-start.error.txt"
  exit 1
fi

ready=false
for _ in {1..120}; do
  if docker exec "$DB_CONTAINER" pg_isready --username="$DB_USER" --dbname="$DB_NAME" \
    > "$CAPTURE_OUTPUT_DIR/environment/pg-isready.stdout" \
    2> "$CAPTURE_OUTPUT_DIR/environment/pg-isready.stderr"; then
    ready=true
    break
  fi
  sleep 1
done
if [[ "$ready" != true ]]; then
  docker logs "$DB_CONTAINER" > "$CAPTURE_OUTPUT_DIR/environment/db-startup.log" 2>&1 || true
  printf 'Perf DB did not become ready within 120 seconds.\n' \
    > "$CAPTURE_OUTPUT_DIR/environment/db-startup.error.txt"
  exit 1
fi

docker inspect --format 'memory_bytes={{.HostConfig.Memory}} memory_swap_bytes={{.HostConfig.MemorySwap}} command={{json .Config.Cmd}}' "$DB_CONTAINER" \
  > "$CAPTURE_OUTPUT_DIR/environment/container-runtime.txt"
docker stats --no-stream --format 'cpu={{.CPUPerc}} memory={{.MemUsage}} memory_percent={{.MemPerc}}' "$DB_CONTAINER" \
  > "$CAPTURE_OUTPUT_DIR/environment/container-stats.txt"

profile_status=0
if docker exec -i "$DB_CONTAINER" psql \
  -X -q -U "$DB_USER" -d "$DB_NAME" \
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
  printf 'The saved synthetic profile did not pass verification; plans were skipped.\n' \
    > "$CAPTURE_OUTPUT_DIR/profile/verify-profile.result.txt"
  exit 1
fi

docker exec "$DB_CONTAINER" psql -X -qAt -U "$DB_USER" -d "$DB_NAME" \
  -c "SELECT jsonb_pretty(to_jsonb(validation)) FROM query_perf_profile_validation AS validation WHERE profile_id = 'synthetic-market-v1'" \
  > "$CAPTURE_OUTPUT_DIR/profile/saved-profile.json" \
  2> "$CAPTURE_OUTPUT_DIR/profile/saved-profile.stderr"
docker exec "$DB_CONTAINER" psql -X -qAt -U "$DB_USER" -d "$DB_NAME" \
  -c "SELECT reference_date::text FROM query_perf_seed_metadata WHERE profile_id = 'synthetic-market-v1'" \
  > "$CAPTURE_OUTPUT_DIR/profile/reference-date.txt" \
  2> "$CAPTURE_OUTPUT_DIR/profile/reference-date.stderr"
docker exec "$DB_CONTAINER" psql -X -qAt -U "$DB_USER" -d "$DB_NAME" \
  -c "SELECT max(timestamp)::date::text FROM bars WHERE timeframe = '1d'" \
  > "$CAPTURE_OUTPUT_DIR/profile/target-date.txt" \
  2> "$CAPTURE_OUTPUT_DIR/profile/target-date.stderr"

reference_date=$(< "$CAPTURE_OUTPUT_DIR/profile/reference-date.txt")
target_date=$(< "$CAPTURE_OUTPUT_DIR/profile/target-date.txt")
if [[ ! "$reference_date" =~ ^[0-9]{4}-[0-9]{2}-[0-9]{2}$ ||
  ! "$target_date" =~ ^[0-9]{4}-[0-9]{2}-[0-9]{2}$ ]]; then
  printf 'Could not read the profile reference date and query target date.\n' \
    > "$CAPTURE_OUTPUT_DIR/profile/date-error.txt"
  exit 1
fi
if [[ "$target_date" != "$EXPECTED_REPLAY_TARGET_DATE" ]]; then
  printf 'Expected target date %s, found %s.\n' "$EXPECTED_REPLAY_TARGET_DATE" "$target_date" \
    > "$CAPTURE_OUTPUT_DIR/profile/target-date-mismatch.txt"
  exit 1
fi
reference_date_epoch=$(date -u -d "$reference_date 00:00:00" +%s)
target_date_epoch=$(date -u -d "$target_date 00:00:00" +%s)
reference_to_target_days=$(((target_date_epoch - reference_date_epoch) / 86400))
printf 'reference_date=%s\nquery_target_date=%s\nreference_to_target_days=%s\n' \
  "$reference_date" "$target_date" "$reference_to_target_days" \
  > "$CAPTURE_OUTPUT_DIR/profile/dates.txt"

docker exec "$DB_CONTAINER" psql -X -q -U "$DB_USER" -d "$DB_NAME" \
  -c "SELECT name, setting, unit, source FROM pg_settings WHERE name IN ('server_version', 'shared_buffers', 'work_mem', 'max_parallel_workers', 'max_parallel_workers_per_gather', 'plan_cache_mode', 'statement_timeout', 'TimeZone', 'effective_cache_size', 'temp_buffers', 'jit', 'random_page_cost', 'effective_io_concurrency') OR name LIKE 'timescaledb.%' ORDER BY name" \
  > "$CAPTURE_OUTPUT_DIR/environment/planner-settings.txt" \
  2> "$CAPTURE_OUTPUT_DIR/environment/planner-settings.stderr"
docker exec "$DB_CONTAINER" psql -X -q -U "$DB_USER" -d "$DB_NAME" \
  -c 'SELECT version(); SELECT extversion FROM pg_extension WHERE extname = '\''timescaledb'\''' \
  > "$CAPTURE_OUTPUT_DIR/environment/database-version.txt" \
  2> "$CAPTURE_OUTPUT_DIR/environment/database-version.stderr"
docker exec "$DB_CONTAINER" psql -X -qAt -U "$DB_USER" -d "$DB_NAME" \
  -c "BEGIN READ ONLY; SET LOCAL statement_timeout = '5s'; SHOW statement_timeout; ROLLBACK" \
  > "$CAPTURE_OUTPUT_DIR/environment/capture-statement-timeout.txt" \
  2> "$CAPTURE_OUTPUT_DIR/environment/capture-statement-timeout.stderr"

run_observation() {
  if ! backend/scripts/capture-group-query-plan-observation.sh "$@"; then
    overall_status=1
  fi
}

bind_index=0
while IFS= read -r bind_json; do
  bind_index=$((bind_index + 1))
  bind_id=$(printf 'bind-%02d' "$bind_index")
  bind_plan_dir="$CAPTURE_OUTPUT_DIR/plans/$bind_id"
  bind_file="$CAPTURE_OUTPUT_DIR/binds/$bind_id-input.json"
  component_result_files=()
  mkdir -p "$bind_plan_dir"
  printf '%s\n' "$bind_json" > "$bind_file"

  run_observation "$bind_id" "$bind_file" 'full.explain' \
    "$CAPTURE_OUTPUT_DIR/sql/full.sql" false
  run_observation "$bind_id" "$bind_file" 'full.analyze' \
    "$CAPTURE_OUTPUT_DIR/sql/full.sql" true

  full_query_timed_out=false
  if jq -s -e '[.[].outcome] | any(. == "timeout")' \
    "$bind_plan_dir/full.explain/result.json" \
    "$bind_plan_dir/full.analyze/result.json" > /dev/null 2>&1; then
    full_query_timed_out=true
  fi

  if [[ "$full_query_timed_out" == true ]]; then
    for component in period_bars latest_period_bars stock_change_prices avg_turnover group_returns ranked_returns; do
      run_observation "$bind_id" "$bind_file" "components/$component.explain" \
        "$CAPTURE_OUTPUT_DIR/sql/$component.sql" false "$component"
      component_result_files+=("$bind_plan_dir/components/$component.explain/result.json")
      run_observation "$bind_id" "$bind_file" "components/$component.analyze" \
        "$CAPTURE_OUTPUT_DIR/sql/$component.sql" true "$component"
      component_result_files+=("$bind_plan_dir/components/$component.analyze/result.json")
    done
  fi

  if ((${#component_result_files[@]} > 0)); then
    jq -s '.' "${component_result_files[@]}" > "$bind_plan_dir/components/results.json"
  else
    jq -n '[]' > "$bind_plan_dir/components/results.json"
  fi
  jq -n \
    --argjson bind "$bind_json" \
    --arg bind_id "$bind_id" \
    --argjson full_query_timed_out "$full_query_timed_out" \
    --slurpfile explain "$bind_plan_dir/full.explain/result.json" \
    --slurpfile analyze "$bind_plan_dir/full.analyze/result.json" \
    --slurpfile components "$bind_plan_dir/components/results.json" \
    '{bind_id: $bind_id, replay_outcome: "not-performed", parameter_types: $bind.parameter_types, original_parameters: $bind.original_parameters, shifted_parameters: $bind.shifted_parameters, full_query_timed_out: $full_query_timed_out, full_query: {explain: $explain[0], analyze: $analyze[0]}, component_observations: $components[0]}' \
    > "$bind_plan_dir/summary.json"
  cp "$bind_plan_dir/summary.json" "$CAPTURE_OUTPUT_DIR/binds/$bind_id.json"
done < <(jq -c '.[]' "$INPUT_GROUP_BINDS_FILE")

if ((bind_index != EXPECTED_GROUP_BIND_COUNT)); then
  printf 'Expected %s bind observations, recorded %s.\n' \
    "$EXPECTED_GROUP_BIND_COUNT" "$bind_index" > "$CAPTURE_OUTPUT_DIR/plans/bind-count-error.txt"
  overall_status=1
else
  binds_recorded=$bind_index
  plans_recorded=true
fi

exit "$overall_status"
