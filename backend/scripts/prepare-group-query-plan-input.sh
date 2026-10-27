#!/usr/bin/env bash
set -euo pipefail

readonly EXPECTED_REPLAY_TARGET_DATE='2026-10-22'
readonly EXPECTED_GROUP_BIND_COUNT=6
readonly SOURCE_CHECKOUT_SHA='d24a3e71dac02684f1d752c4d37aeb6462ca5c91'
readonly REFERENCE_MANIFEST_FILE='backend/scripts/query-perf-input-manifest/reference-manifest.json'
readonly SOURCE_SQL_PATH='backend/crates/gateways/postgres/src/group_movers.rs'
readonly SOURCE_SQL_HELPER_PATH='backend/crates/gateways/postgres/src/group_query_sql.rs'

QUERY_LOG_FILE=$1
INPUT_ARTIFACT_NAME=$2
INPUT_ARTIFACT_ID=$3
INPUT_MANIFEST_FILE=$4
INPUT_COMPARISON_FILE=$5
INPUT_GROUP_BINDS_FILE=$6
CAPTURE_OUTPUT_DIR=$7

mkdir -p "$CAPTURE_OUTPUT_DIR"/{input,sql,binds}
gate_file="$CAPTURE_OUTPUT_DIR/input/group-plan-gate.json"

if [[ -z "$INPUT_ARTIFACT_NAME" || -z "$INPUT_ARTIFACT_ID" ]]; then
  printf 'Regenerated Actions artifact metadata is missing.\n' \
    > "$CAPTURE_OUTPUT_DIR/input/artifact-metadata-error.txt"
  exit 1
fi
if [[ ! -s "$QUERY_LOG_FILE" || ! -s "$INPUT_MANIFEST_FILE" ||
  ! -s "$INPUT_COMPARISON_FILE" || ! -s "$INPUT_GROUP_BINDS_FILE" ]]; then
  printf 'A regenerated query log, manifest, comparison, or group bind set is missing.\n' \
    > "$CAPTURE_OUTPUT_DIR/input/group-plan-gate-error.txt"
  exit 1
fi

manifest_matches=false
if jq -e \
  --arg source_checkout_sha "$SOURCE_CHECKOUT_SHA" \
  --arg target_date "$EXPECTED_REPLAY_TARGET_DATE" \
  --slurpfile reference "$REFERENCE_MANIFEST_FILE" \
  '.provenance.input_kind == "ci-regenerated" and
   .provenance.source_checkout_sha == $source_checkout_sha and
   .comparison.target_date == $target_date and
   .comparison.group_movers == $reference[0].comparison.group_movers and
   .comparison.group_movers.bind_count == 6 and
   .comparison.group_movers.specified_bind_count == 4' \
  "$INPUT_MANIFEST_FILE" > "$CAPTURE_OUTPUT_DIR/input/group-manifest-validation.stdout" \
  2> "$CAPTURE_OUTPUT_DIR/input/group-manifest-validation.stderr" &&
  jq -e '.group_movers_match == true and .target_date_matches == true' \
    "$INPUT_COMPARISON_FILE" > "$CAPTURE_OUTPUT_DIR/input/group-comparison-validation.stdout" \
    2> "$CAPTURE_OUTPUT_DIR/input/group-comparison-validation.stderr"; then
  manifest_matches=true
fi

binds_match=false
if jq -e \
  'length == 6 and
   all(.[]; .parameter_types == ["text", "date", "date", "numeric", "bigint"] and
       (.original_parameters | length) == 5 and
       (.shifted_parameters | length) == 5)' \
  "$INPUT_GROUP_BINDS_FILE" > "$CAPTURE_OUTPUT_DIR/input/group-binds-validation.stdout" \
  2> "$CAPTURE_OUTPUT_DIR/input/group-binds-validation.stderr"; then
  binds_match=true
fi

jq -n \
  --argjson manifest_matches "$manifest_matches" \
  --argjson binds_match "$binds_match" \
  --argjson full_input_matches "$(jq -r '.matches // false' "$INPUT_COMPARISON_FILE")" \
  --arg expected_sql_sha256 "$(jq -r '.comparison.group_movers.sql_sha256' "$REFERENCE_MANIFEST_FILE")" \
  --arg actual_sql_sha256 "$(jq -r '.comparison.group_movers.sql_sha256 // "unavailable"' "$INPUT_MANIFEST_FILE")" \
  --arg expected_parameter_types_sha256 "$(jq -r '.comparison.group_movers.parameter_types_sha256' "$REFERENCE_MANIFEST_FILE")" \
  --arg actual_parameter_types_sha256 "$(jq -r '.comparison.group_movers.parameter_types_sha256 // "unavailable"' "$INPUT_MANIFEST_FILE")" \
  --arg expected_parameter_types_set_sha256 "$(jq -r '.comparison.group_movers.parameter_types_set_sha256' "$REFERENCE_MANIFEST_FILE")" \
  --arg actual_parameter_types_set_sha256 "$(jq -r '.comparison.group_movers.parameter_types_set_sha256 // "unavailable"' "$INPUT_MANIFEST_FILE")" \
  --arg expected_typed_bind_set_sha256 "$(jq -r '.comparison.group_movers.typed_shifted_bind_set_sha256' "$REFERENCE_MANIFEST_FILE")" \
  --arg actual_typed_bind_set_sha256 "$(jq -r '.comparison.group_movers.typed_shifted_bind_set_sha256 // "unavailable"' "$INPUT_MANIFEST_FILE")" \
  --argjson expected_bind_count "$EXPECTED_GROUP_BIND_COUNT" \
  --argjson actual_bind_count "$(jq -r '.comparison.group_movers.bind_count // 0' "$INPUT_MANIFEST_FILE")" \
  '((.manifest_matches == true) and (.binds_match == true)) as $matched |
   {
     status: (if $matched then "passed" else "failed" end),
     full_input_equivalence_matches: .full_input_matches,
     group_sql_matches: (.expected_sql_sha256 == .actual_sql_sha256),
     parameter_types_match: (.expected_parameter_types_sha256 == .actual_parameter_types_sha256 and .expected_parameter_types_set_sha256 == .actual_parameter_types_set_sha256),
     all_six_typed_binds_match: (.expected_bind_count == .actual_bind_count and .expected_typed_bind_set_sha256 == .actual_typed_bind_set_sha256 and .binds_match == true),
     bind_count: .actual_bind_count,
     target_date: "2026-10-22",
     expected_sql_sha256: .expected_sql_sha256,
     actual_sql_sha256: .actual_sql_sha256,
     expected_parameter_types_sha256: .expected_parameter_types_sha256,
     actual_parameter_types_sha256: .actual_parameter_types_sha256,
     expected_parameter_types_set_sha256: .expected_parameter_types_set_sha256,
     actual_parameter_types_set_sha256: .actual_parameter_types_set_sha256,
     expected_typed_bind_set_sha256: .expected_typed_bind_set_sha256,
     actual_typed_bind_set_sha256: .actual_typed_bind_set_sha256
   }' > "$gate_file"

if [[ "$manifest_matches" != true || "$binds_match" != true ]]; then
  printf 'Group SQL, typed binds, or target date did not match the reviewed reference; plans were skipped.\n' \
    > "$CAPTURE_OUTPUT_DIR/input/group-plan-gate-error.txt"
  exit 1
fi

if ! jq -s -e '
  [ .[] | select((.message // "") | contains("eligible_stocks AS") and contains("period_bars AS MATERIALIZED")) ] as $rows
  | if ($rows | length) == 0 then error("no group query rows found")
    elif ([$rows[] | .message | sub("^execute [^:]+: "; "")] | unique | length) != 1 then error("group query SQL differs between log rows")
    else true end
' "$QUERY_LOG_FILE" > "$CAPTURE_OUTPUT_DIR/sql/source-query-validation.stdout" \
  2> "$CAPTURE_OUTPUT_DIR/sql/source-query-validation.stderr"; then
  printf 'Could not uniquely identify the matched group SQL from the regenerated log.\n' \
    > "$CAPTURE_OUTPUT_DIR/sql/source-query-validation.error.txt"
  exit 1
fi

for required_source in \
  'static LIST_GROUP_MOVERS_SQL' \
  'eligible_stocks AS (' \
  'period_bars AS MATERIALIZED (' \
  'latest_period_bars AS (' \
  'stock_change_prices AS (' \
  'avg_turnover AS MATERIALIZED (' \
  'group_returns AS (' \
  'ranked_returns AS ('; do
  if ! grep -Fq -- "$required_source" "$SOURCE_SQL_PATH"; then
    printf 'Source query does not contain expected SQL fragment: %s\n' "$required_source" \
      >> "$CAPTURE_OUTPUT_DIR/sql/source-query-validation.error.txt"
    exit 1
  fi
done

jq -s -er '
  [ .[] | select((.message // "") | contains("eligible_stocks AS") and contains("period_bars AS MATERIALIZED")) | .message | sub("^execute [^:]+: "; "") ]
  | unique
  | if length == 1 then .[0] else error("group SQL was not unique") end
' "$QUERY_LOG_FILE" > "$CAPTURE_OUTPUT_DIR/sql/full.sql"
if ! grep -Eq '^WITH requested_axes AS' "$CAPTURE_OUTPUT_DIR/sql/full.sql"; then
  printf 'Extracted group SQL does not start with the application CTE chain.\n' \
    > "$CAPTURE_OUTPUT_DIR/sql/source-query-validation.error.txt"
  exit 1
fi
if git diff --quiet "$SOURCE_CHECKOUT_SHA" HEAD -- "$SOURCE_SQL_PATH" "$SOURCE_SQL_HELPER_PATH"; then
  cp "$SOURCE_SQL_PATH" "$CAPTURE_OUTPUT_DIR/sql/group_movers.rs"
  cp "$SOURCE_SQL_HELPER_PATH" "$CAPTURE_OUTPUT_DIR/sql/group_query_sql.rs"
  printf 'Application SQL source files match the fixed checkout.\n' \
    > "$CAPTURE_OUTPUT_DIR/sql/source-files-check.txt"
else
  printf 'Application SQL source files differ from the fixed checkout.\n' \
    > "$CAPTURE_OUTPUT_DIR/sql/source-files-check.txt"
  exit 1
fi

perl -0777 -ne 's/SELECT group_metrics\.group_key,.*\z//s or die "could not locate the group query final SELECT\n"; print' \
  "$CAPTURE_OUTPUT_DIR/sql/full.sql" > "$CAPTURE_OUTPUT_DIR/sql/cte-prefix.sql"
if ! grep -Eq '^WITH requested_axes AS' "$CAPTURE_OUTPUT_DIR/sql/cte-prefix.sql"; then
  printf 'Extracted SQL prefix does not begin with the application CTE chain.\n' \
    > "$CAPTURE_OUTPUT_DIR/sql/component-error.txt"
  exit 1
fi
for component in period_bars latest_period_bars stock_change_prices avg_turnover group_returns ranked_returns; do
  {
    cat "$CAPTURE_OUTPUT_DIR/sql/cte-prefix.sql"
    printf 'SELECT * FROM %s\n' "$component"
  } > "$CAPTURE_OUTPUT_DIR/sql/$component.sql"
done
