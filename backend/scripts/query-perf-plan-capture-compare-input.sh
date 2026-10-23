#!/usr/bin/env bash
set -euo pipefail

input_dir="$CAPTURE_OUTPUT_DIR/input"
mkdir -p "$input_dir"

if [[ ! -s "$QUERY_LOG_PATH" ]]; then
  echo "matches=false" >> "$GITHUB_OUTPUT"
  jq -n \
    --arg kind "query_log_missing" \
    --arg detail "Regenerated PostgreSQL query log is missing or empty." \
    '{status: "failed", failure_kind: $kind, difference_count: 1, detail: $detail}' \
    > "$input_dir/failure.json"
  exit 1
fi
if [[ -z "$INPUT_ARTIFACT_ID" ]]; then
  echo "matches=false" >> "$GITHUB_OUTPUT"
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
  echo "matches=false" >> "$GITHUB_OUTPUT"
  jq -n \
    --arg kind "migration_database_lookup_failure" \
    --arg detail "Could not identify the migrated fixed-source test database." \
    '{status: "failed", failure_kind: $kind, difference_count: 1, detail: $detail}' \
    > "$input_dir/failure.json"
  exit 1
fi
if [[ ! "$test_database" =~ ^t_rader_test_ready_[0-9a-f]{32}$ ]]; then
  echo "matches=false" >> "$GITHUB_OUTPUT"
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
  echo "matches=true" >> "$GITHUB_OUTPUT"
  jq -n \
    --arg artifact_name "$INPUT_ARTIFACT_NAME" \
    --arg artifact_id "$INPUT_ARTIFACT_ID" \
    --arg input_log_sha256 "$(sha256sum "$QUERY_LOG_PATH" | cut -d ' ' -f 1)" \
    '{status: "passed", input_kind: "ci-regenerated", artifact_name: $artifact_name, artifact_id: $artifact_id, input_log_sha256: $input_log_sha256}' \
    > "$input_dir/result.json"
else
  echo "matches=false" >> "$GITHUB_OUTPUT"
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
