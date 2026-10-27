#!/usr/bin/env bash
set -euo pipefail

readonly DB_CONTAINER='query-perf-group-plan-capture-db'
readonly DB_NAME='t_rader'
readonly DB_USER='t_rader'

bind_id=$1
bind_file=$2
observation=$3
query_file=$4
analyze=$5
component=${6:-}
readonly CAPTURE_OUTPUT_DIR=${CAPTURE_OUTPUT_DIR:-query-perf-plan-capture}
plan_dir="$CAPTURE_OUTPUT_DIR/plans/$bind_id/$observation"
psql_file="$plan_dir/statement.psql"
stdout_file="$plan_dir/stdout.txt"
stderr_file="$plan_dir/stderr.txt"
command_status=0
elapsed_ms=0
sqlstate=''
outcome='error'
mkdir -p "$plan_dir"

record_result() {
  printf '%s\n' "$command_status" > "$plan_dir/exit_code"
  printf '%s\n' "$elapsed_ms" > "$plan_dir/elapsed_ms"
  printf '%s\n' "${sqlstate:-unavailable}" > "$plan_dir/sqlstate"
  jq -n \
    --arg bind_id "$bind_id" \
    --arg observation "$observation" \
    --arg component "$component" \
    --arg outcome "$outcome" \
    --arg sqlstate "$sqlstate" \
    --argjson exit_code "$command_status" \
    --argjson elapsed_ms "$elapsed_ms" \
    '{bind_id: $bind_id, observation: $observation, component: (if $component == "" then null else $component end), outcome: $outcome, statement_timeout: "5s", read_only: true, sqlstate: (if $sqlstate == "" then null else $sqlstate end), exit_code: $exit_code, elapsed_ms: $elapsed_ms}' \
    > "$plan_dir/result.json"
  printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\n' \
    "$bind_id" "$observation" "${component:-full}" "$command_status" \
    "$elapsed_ms" "${sqlstate:-unavailable}" "$outcome" \
    >> "$CAPTURE_OUTPUT_DIR/observation-order.tsv"
}

sql_literal() {
  local parameter_index=$1
  local parameter_type=$2
  local value_json
  local value
  local escaped
  value_json=$(jq -c ".shifted_parameters[$parameter_index]" "$bind_file")
  if [[ "$value_json" == 'null' ]]; then
    printf 'NULL'
    return 0
  fi
  value=$(jq -r ".shifted_parameters[$parameter_index]" "$bind_file")
  case "$parameter_type" in
    text)
      escaped=${value//\'/\'\'}
      printf "'%s'" "$escaped"
      ;;
    date)
      if [[ ! "$value" =~ ^[0-9]{4}-[0-9]{2}-[0-9]{2}$ ]]; then
        return 1
      fi
      printf "'%s'" "$value"
      ;;
    numeric)
      if [[ ! "$value" =~ ^[+-]?([0-9]+([.][0-9]*)?|[.][0-9]+)([eE][+-]?[0-9]+)?$ ]]; then
        return 1
      fi
      printf '%s' "$value"
      ;;
    bigint)
      if [[ ! "$value" =~ ^[+-]?[0-9]+$ ]]; then
        return 1
      fi
      printf '%s' "$value"
      ;;
    *)
      return 1
      ;;
  esac
}

explain_options='FORMAT JSON'
if [[ "$analyze" == true ]]; then
  explain_options='ANALYZE, BUFFERS, FORMAT JSON'
elif [[ "$analyze" != false ]]; then
  printf 'analyze must be true or false\n' > "$stderr_file"
  command_status=2
  outcome='invalid_observation'
  record_result
  exit 1
fi

literal_axis=$(sql_literal 0 text) || command_status=2
literal_from_date=$(sql_literal 1 date) || command_status=2
literal_to_date=$(sql_literal 2 date) || command_status=2
literal_min_turnover=$(sql_literal 3 numeric) || command_status=2
literal_limit=$(sql_literal 4 bigint) || command_status=2
if ((command_status != 0)); then
  printf 'A shifted bind value did not match its resolved PostgreSQL type.\n' \
    > "$stderr_file"
  outcome='bind_literal_validation_failed'
  record_result
  exit 1
fi

{
  printf 'BEGIN READ ONLY;\n'
  printf "SET LOCAL statement_timeout = '5s';\n"
  printf 'PREPARE capture_statement(text, date, date, numeric, bigint) AS\n'
  cat "$query_file"
  printf ';\n'
  printf 'EXPLAIN (%s) EXECUTE capture_statement(%s, %s, %s, %s, %s);\n' \
    "$explain_options" "$literal_axis" "$literal_from_date" "$literal_to_date" \
    "$literal_min_turnover" "$literal_limit"
  printf 'ROLLBACK;\n'
} > "$psql_file"

started=$(date +%s%N)
if docker exec -i "$DB_CONTAINER" psql \
  -X -qAt -U "$DB_USER" -d "$DB_NAME" \
  -v ON_ERROR_STOP=1 -v VERBOSITY=verbose \
  < "$psql_file" > "$stdout_file" 2> "$stderr_file"; then
  command_status=0
else
  command_status=$?
fi
elapsed_ms=$((($(date +%s%N) - started) / 1000000))
sqlstate=$(sed -nE 's/^ERROR:[[:space:]]*([0-9A-Z]{5}):.*/\1/p' "$stderr_file" | head -n 1)

if ((command_status == 0)) && jq -e 'length == 1 and (.[0] | has("Plan"))' "$stdout_file" > /dev/null 2>&1; then
  outcome='plan'
  cp "$stdout_file" "$plan_dir/plan.json"
elif [[ "$sqlstate" == '57014' ]] && grep -Eiq 'statement timeout' "$stderr_file"; then
  outcome='timeout'
else
  outcome='error'
  if ((command_status == 0)); then
    command_status=1
  fi
  record_result
  exit 1
fi

record_result
