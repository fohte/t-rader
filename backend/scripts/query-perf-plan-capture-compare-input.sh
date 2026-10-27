#!/usr/bin/env bash
set -euo pipefail

readonly REFERENCE_MANIFEST_FILE='backend/scripts/query-perf-input-manifest/reference-manifest.json'
readonly EXPECTED_REPLAY_TARGET_DATE='2026-10-22'
readonly SOURCE_CHECKOUT_SHA='d24a3e71dac02684f1d752c4d37aeb6462ca5c91'

input_dir="$CAPTURE_OUTPUT_DIR/input"
mkdir -p "$input_dir"
matches=false
group_matches=false

write_outputs() {
  local command_status=$?
  if [[ -s "$input_dir/regenerated-manifest.json" && -s "$input_dir/comparison.json" ]] &&
    jq -e \
      --arg source_checkout_sha "$SOURCE_CHECKOUT_SHA" \
      --arg target_date "$EXPECTED_REPLAY_TARGET_DATE" \
      --slurpfile reference "$REFERENCE_MANIFEST_FILE" \
      '.provenance.input_kind == "ci-regenerated" and
       .provenance.source_checkout_sha == $source_checkout_sha and
       .comparison.target_date == $target_date and
       .comparison.group_movers == $reference[0].comparison.group_movers and
       .comparison.group_movers.bind_count == 6 and
       .comparison.group_movers.specified_bind_count == 4' \
      "$input_dir/regenerated-manifest.json" > /dev/null 2>&1 &&
    jq -e '.group_movers_match == true and .target_date_matches == true' \
      "$input_dir/comparison.json" > /dev/null 2>&1; then
    group_matches=true
  fi
  if [[ -n "${GITHUB_OUTPUT:-}" ]]; then
    printf 'matches=%s\ngroup_matches=%s\n' "$matches" "$group_matches" >> "$GITHUB_OUTPUT"
  fi
  return "$command_status"
}
trap write_outputs EXIT

if [[ ! -s "$QUERY_LOG_PATH" ]]; then
  jq -n \
    --arg kind "query_log_missing" \
    --arg detail "Regenerated PostgreSQL query log is missing or empty." \
    '{status: "failed", failure_kind: $kind, difference_count: 1, detail: $detail}' \
    > "$input_dir/failure.json"
  exit 1
fi
if [[ -z "$INPUT_ARTIFACT_ID" ]]; then
  jq -n \
    --arg kind "query_log_artifact_missing" \
    --arg detail "Regenerated PostgreSQL query log artifact ID is unavailable." \
    '{status: "failed", failure_kind: $kind, difference_count: 1, detail: $detail}' \
    > "$input_dir/failure.json"
  exit 1
fi

if ! test_database=$(docker exec "$DB_CONTAINER_ID" psql \
  --username=t_rader \
  --dbname=postgres \
  --set=ON_ERROR_STOP=1 \
  --tuples-only \
  --no-align \
  --command="SELECT datname FROM pg_database WHERE datname ~ '^t_rader_test_ready_[0-9a-f]{32}$' ORDER BY datname"); then
  jq -n \
    --arg kind "migration_database_lookup_failure" \
    --arg detail "Could not identify the migrated fixed-source test database." \
    '{status: "failed", failure_kind: $kind, difference_count: 1, detail: $detail}' \
    > "$input_dir/failure.json"
  exit 1
fi
if [[ ! "$test_database" =~ ^t_rader_test_ready_[0-9a-f]{32}$ ]]; then
  jq -n \
    --arg kind "migration_database_lookup_failure" \
    --arg detail "Expected exactly one migrated fixed-source test database." \
    '{status: "failed", failure_kind: $kind, difference_count: 1, detail: $detail}' \
    > "$input_dir/failure.json"
  exit 1
fi
printf '%s\n' "$test_database" > "$input_dir/preflight-database.txt"

command_status=0
DATABASE_URL="postgres://t_rader:t_rader@localhost:5432/$test_database" \
  cargo run --locked --manifest-path backend/scripts/query-perf-input-manifest/Cargo.toml -- \
  build \
  --log-file "$QUERY_LOG_PATH" \
  --target-date 2026-10-22 \
  --source-checkout-sha "$SOURCE_CHECKOUT_SHA" \
  --input-kind ci-regenerated \
  --input-artifact-name "$INPUT_ARTIFACT_NAME" \
  --input-artifact-id "$INPUT_ARTIFACT_ID" \
  --exclusions-file backend/crates/apps/query-perf/exclusions.json \
  --output "$input_dir/regenerated-manifest.json" \
  --group-binds-output "$CAPTURE_OUTPUT_DIR/binds/source-binds.json" \
  --compare-to backend/scripts/query-perf-input-manifest/reference-manifest.json \
  --comparison-output "$input_dir/comparison.json" \
  --failure-output "$input_dir/failure.json" \
  > "$input_dir/comparison.stdout" \
  2> "$input_dir/comparison.stderr" || command_status=$?

if ((command_status == 0)) && jq -e '.matches == true' "$input_dir/comparison.json" > /dev/null; then
  matches=true
  jq -n \
    --arg artifact_name "$INPUT_ARTIFACT_NAME" \
    --arg artifact_id "$INPUT_ARTIFACT_ID" \
    --arg input_log_sha256 "$(sha256sum "$QUERY_LOG_PATH" | cut -d ' ' -f 1)" \
    '{status: "passed", input_kind: "ci-regenerated", artifact_name: $artifact_name, artifact_id: $artifact_id, input_log_sha256: $input_log_sha256}' \
    > "$input_dir/result.json"
else
  if [[ ! -s "$input_dir/failure.json" ]]; then
    jq -n \
      --arg kind "full_input_comparison_failed" \
      --arg detail "The full SQL, typed bind, or exclusion comparison did not pass." \
      --argjson command_status "$command_status" \
      '{status: "failed", failure_kind: $kind, difference_count: 1, command_status: $command_status, detail: $detail}' \
      > "$input_dir/failure.json"
  fi
  jq -n \
    --arg artifact_name "$INPUT_ARTIFACT_NAME" \
    --arg artifact_id "$INPUT_ARTIFACT_ID" \
    --arg input_log_sha256 "$(sha256sum "$QUERY_LOG_PATH" | cut -d ' ' -f 1)" \
    --argjson command_status "$command_status" \
    '{status: "failed", input_kind: "ci-regenerated", artifact_name: $artifact_name, artifact_id: $artifact_id, input_log_sha256: $input_log_sha256, command_status: $command_status}' \
    > "$input_dir/result.json"
  exit 1
fi
