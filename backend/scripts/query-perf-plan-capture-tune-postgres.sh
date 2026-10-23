#!/usr/bin/env bash
set -euo pipefail

db_psql() {
  docker exec "$DB_CONTAINER_ID" psql \
    --username=t_rader \
    --dbname=t_rader_test \
    --set=ON_ERROR_STOP=1 \
    "$@"
}

dump_db_logs() {
  echo "::group::PostgreSQL service logs"
  docker logs "$DB_CONTAINER_ID" || true
  docker exec "$DB_CONTAINER_ID" sh -c '
    for log_file in "$PGDATA"/log/*.json; do
      [ -f "$log_file" ] || continue
      cat "$log_file"
    done
  ' || true
  echo "::endgroup::"
}

db_psql \
  --command="ALTER SYSTEM SET fsync = off" \
  --command="ALTER SYSTEM SET synchronous_commit = off" \
  --command="ALTER SYSTEM SET full_page_writes = off" \
  --command="ALTER SYSTEM SET timescaledb.max_background_workers = 0" \
  --command="ALTER SYSTEM SET logging_collector = on" \
  --command="ALTER SYSTEM SET log_destination = 'jsonlog'" \
  --command="ALTER SYSTEM SET log_statement = 'all'" \
  --command="ALTER SYSTEM SET log_parameter_max_length = -1" \
  --command="ALTER SYSTEM SET log_rotation_age = 0" \
  --command="ALTER SYSTEM SET log_rotation_size = 0"
docker restart "$DB_CONTAINER_ID"
ready=false
for _ in {1..60}; do
  if db_psql --command="SELECT 1" > /dev/null 2>&1; then
    ready=true
    break
  fi
  sleep 1
done
if [[ "$ready" != true ]]; then
  echo "PostgreSQL did not become ready after restart" >&2
  dump_db_logs
  exit 1
fi
settings=$(db_psql \
  --tuples-only \
  --no-align \
  --command="SHOW fsync" \
  --command="SHOW synchronous_commit" \
  --command="SHOW full_page_writes" \
  --command="SHOW timescaledb.max_background_workers")
if [[ "$settings" != $'off\noff\noff\n0' ]]; then
  printf 'PostgreSQL test settings do not match expectations:\n%s\n' "$settings" >&2
  dump_db_logs
  exit 1
fi
log_settings=$(db_psql \
  --tuples-only \
  --no-align \
  --command="SHOW logging_collector" \
  --command="SHOW log_destination" \
  --command="SHOW log_statement" \
  --command="SHOW log_parameter_max_length" \
  --command="SHOW log_rotation_age" \
  --command="SHOW log_rotation_size")
if [[ "$log_settings" != $'on\njsonlog\nall\n-1\n0\n0' ]]; then
  printf 'PostgreSQL query logging settings do not match expectations:\n%s\n' "$log_settings" >&2
  dump_db_logs
  exit 1
fi
timescaledb_workers=$(db_psql \
  --tuples-only \
  --no-align \
  --command="SELECT count(*) FROM pg_stat_activity WHERE application_name LIKE '%TimescaleDB%'")
if [[ "$timescaledb_workers" != "0" ]]; then
  printf 'TimescaleDB background workers are still running: %s\n' "$timescaledb_workers" >&2
  dump_db_logs
  exit 1
fi
