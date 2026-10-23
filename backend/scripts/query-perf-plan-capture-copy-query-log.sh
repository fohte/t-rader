#!/usr/bin/env bash
set -euo pipefail

QUERY_LOG_PATH="$RUNNER_TEMP/backend-query-log-regenerated/postgresql.json"

db_psql() {
  docker exec "$DB_CONTAINER_ID" psql \
    --username=t_rader \
    --dbname=t_rader_test \
    --set=ON_ERROR_STOP=1 \
    --tuples-only \
    --no-align \
    "$@"
}

if ! log_path=$(db_psql --command="SELECT current_setting('data_directory') || '/' || pg_current_logfile('jsonlog')"); then
  echo "PostgreSQL query log path could not be read" >&2
  docker logs "$DB_CONTAINER_ID" || true
  exit 1
fi
if [[ -z "$log_path" ]]; then
  echo "PostgreSQL did not report an active JSON log file" >&2
  docker logs "$DB_CONTAINER_ID" || true
  exit 1
fi

mkdir -p "$(dirname "$QUERY_LOG_PATH")"
if ! docker cp "$DB_CONTAINER_ID:$log_path" "$QUERY_LOG_PATH"; then
  echo "PostgreSQL query log could not be copied" >&2
  docker logs "$DB_CONTAINER_ID" || true
  exit 1
fi
