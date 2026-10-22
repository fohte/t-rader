#!/usr/bin/env bash
set -euo pipefail

readonly BASE_IMAGE='timescale/timescaledb:2.26.4-pg18@sha256:9ecddf2c88356f01f2986a0349ea525a5f7c56b81935bbfd58e9590c93f7d859'
readonly IMAGE_REPOSITORY='ghcr.io/fohte/t-rader/perf-db'
readonly POSTGRES_USER='t_rader'
readonly POSTGRES_PASSWORD='t_rader'
readonly POSTGRES_DATABASE='t_rader'

push_image=false
print_image_ref=false
if [[ $# -eq 1 ]]; then
  case "$1" in
    --push) push_image=true ;;
    --image-ref) print_image_ref=true ;;
    *)
      echo "Usage: $0 [--push|--image-ref]" >&2
      exit 2
      ;;
  esac
elif [[ $# -gt 0 ]]; then
  echo "Usage: $0 [--push|--image-ref]" >&2
  exit 2
fi

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd -- "$script_dir/../.." && pwd)"
seed_sql="$repo_root/backend/perf-db/seed.sql"
profile_sql="$repo_root/backend/perf-db/verify-profile.sql"

source_hash=$(
  {
    find "$repo_root/backend/migration" -type f -print
    printf '%s\n' "$seed_sql" "$profile_sql" "$script_dir/build-perf-db-image.sh"
  } |
    LC_ALL=C sort |
    while IFS= read -r path; do
      printf '%s %s\n' "${path#"$repo_root"/}" "$(git -C "$repo_root" hash-object -- "$path")"
    done |
    openssl dgst -sha256 |
    awk '{print $NF}'
)
image_ref="$IMAGE_REPOSITORY:$source_hash"
if [[ "$print_image_ref" == true ]]; then
  echo "$image_ref"
  exit 0
fi
container_name="t-rader-perf-db-build-$$"
container_started=false
build_started_seconds=$SECONDS

if [[ "$push_image" == true ]]; then
  if manifest_output=$(docker manifest inspect "$image_ref" 2>&1); then
    printf 'Perf DB image %s already exists; skipping build and push.\n' "$image_ref"
    exit 0
  else
    case "$manifest_output" in
      *'no such manifest'* | *'manifest unknown'* | *'404 Not Found'*)
        printf 'Perf DB image %s was not found; building and pushing.\n' "$image_ref"
        ;;
      *)
        printf 'Failed to inspect perf DB image %s:\n%s\n' "$image_ref" "$manifest_output" >&2
        exit 1
        ;;
    esac
  fi
fi

cleanup() {
  if [[ "$container_started" == true ]]; then
    docker stop --timeout 60 "$container_name" > /dev/null 2>&1 || true
    docker rm "$container_name" > /dev/null 2>&1 || true
  fi

  local elapsed_seconds=$((SECONDS - build_started_seconds))
  printf 'Perf DB image build elapsed: %s seconds.\n' "$elapsed_seconds"
  if [[ -n "${GITHUB_STEP_SUMMARY:-}" ]]; then
    printf '\n### Perf DB image build\n\nElapsed: %s seconds\n' "$elapsed_seconds" >> "$GITHUB_STEP_SUMMARY"
  fi
}
trap cleanup EXIT

psql_exec() {
  docker exec "$container_name" psql \
    -X \
    -q \
    -U "$POSTGRES_USER" \
    -d "$POSTGRES_DATABASE" \
    -v ON_ERROR_STOP=1 \
    "$@"
}

psql_value() {
  psql_exec -A -t "$@"
}

wal_dir_bytes() {
  psql_value -c 'SELECT coalesce(sum(size), 0) FROM pg_ls_waldir()'
}

wait_for_postgres() {
  local readiness_attempt=0
  for ((readiness_attempt = 0; readiness_attempt < 120; readiness_attempt++)); do
    if psql_exec -c 'SELECT 1' > /dev/null 2>&1; then
      return 0
    fi
    sleep 1
  done

  echo 'PostgreSQL did not become ready within 120 seconds.' >&2
  docker logs "$container_name" >&2
  return 1
}

shrink_wal() {
  local reload_attempt=0 shrink_round=0 checkpoint_cycle=0
  local wal_settings
  psql_exec \
    -c "ALTER SYSTEM SET max_wal_size = '64MB'" \
    -c "ALTER SYSTEM SET min_wal_size = '32MB'" \
    -c 'SELECT pg_reload_conf()' > /dev/null

  for ((reload_attempt = 0; reload_attempt < 30; reload_attempt++)); do
    wal_settings=$(psql_value \
      -c "SELECT current_setting('max_wal_size') || '/' || current_setting('min_wal_size')")
    if [[ "$wal_settings" == '64MB/32MB' ]]; then
      break
    fi
    sleep 1
  done
  if [[ "$wal_settings" != '64MB/32MB' ]]; then
    printf 'WAL settings were not reloaded: %s\n' "$wal_settings" >&2
    return 1
  fi

  # PostgreSQL は直近の checkpoint cycle に基づいて WAL を再利用するため、小さい設定で複数 cycle を回して余分な segment を解放する。
  local max_wal_bytes=67108864
  local wal_bytes
  wal_bytes=$(wal_dir_bytes)
  for ((shrink_round = 0; shrink_round < 12; shrink_round++)); do
    if ((wal_bytes <= max_wal_bytes)); then
      break
    fi
    local checkpoint_args=()
    for ((checkpoint_cycle = 0; checkpoint_cycle < 10; checkpoint_cycle++)); do
      checkpoint_args+=(-c 'SELECT pg_switch_wal()' -c 'CHECKPOINT')
    done
    psql_exec "${checkpoint_args[@]}" > /dev/null
    wal_bytes=$(wal_dir_bytes)
  done
  if ((wal_bytes > max_wal_bytes)); then
    printf 'WAL size did not shrink to 64MB: %s bytes\n' "$wal_bytes" >&2
    return 1
  fi

  psql_exec \
    -c 'ALTER SYSTEM RESET max_wal_size' \
    -c 'ALTER SYSTEM RESET min_wal_size' \
    -c 'SELECT pg_reload_conf()' > /dev/null
  local wal_settings_override
  wal_settings_override=$(psql_value \
    -c "SELECT bool_and(sourcefile IS DISTINCT FROM (current_setting('data_directory') || '/postgresql.auto.conf')) FROM pg_settings WHERE name IN ('max_wal_size', 'min_wal_size')")
  if [[ "$wal_settings_override" != 't' ]]; then
    printf 'Build WAL settings remain in postgresql.auto.conf: %s\n' "$wal_settings_override" >&2
    return 1
  fi

  printf 'WAL size: %s bytes; max_wal_size and min_wal_size restored to defaults.\n' "$wal_bytes"
}

docker run \
  --detach \
  --name "$container_name" \
  --publish 127.0.0.1::5432 \
  --env 'PGDATA=/pgdata' \
  --env "POSTGRES_USER=$POSTGRES_USER" \
  --env "POSTGRES_PASSWORD=$POSTGRES_PASSWORD" \
  --env "POSTGRES_DB=$POSTGRES_DATABASE" \
  "$BASE_IMAGE" \
  -c shared_buffers=128MB \
  -c work_mem=4MB \
  -c maintenance_work_mem=64MB \
  -c timescaledb.max_background_workers=0 \
  -c max_parallel_workers=0 \
  -c max_parallel_workers_per_gather=0 \
  -c autovacuum_max_workers=2 > /dev/null
container_started=true

wait_for_postgres

docker exec "$container_name" psql \
  -U "$POSTGRES_USER" \
  -d "$POSTGRES_DATABASE" \
  -v ON_ERROR_STOP=1 \
  -c "ALTER SYSTEM SET max_wal_size = '1GB'" \
  -c "ALTER SYSTEM SET min_wal_size = '32MB'" \
  -c "ALTER SYSTEM SET timescaledb.max_background_workers = '0'" \
  -c 'SELECT pg_reload_conf()' > /dev/null

port="$(docker port "$container_name" 5432/tcp | sed 's/.*://')"
(
  cd "$repo_root/backend"
  DATABASE_URL="postgres://$POSTGRES_USER:$POSTGRES_PASSWORD@127.0.0.1:$port/$POSTGRES_DATABASE" \
    cargo run -p migration -- up
)

{
  cat "$seed_sql"
  printf '\n'
  cat "$profile_sql"
} | docker exec -i "$container_name" psql \
  -X \
  -q \
  -U "$POSTGRES_USER" \
  -d "$POSTGRES_DATABASE" \
  -v ON_ERROR_STOP=1 \
  -v record_profile=true

shrink_wal
docker stop --timeout 60 "$container_name" > /dev/null
docker commit \
  --change 'ENV PGDATA=/pgdata' \
  --change "ENV POSTGRES_USER=$POSTGRES_USER" \
  --change "ENV POSTGRES_PASSWORD=$POSTGRES_PASSWORD" \
  --change "ENV POSTGRES_DB=$POSTGRES_DATABASE" \
  --change 'LABEL org.opencontainers.image.source=https://github.com/fohte/t-rader' \
  --change 'CMD ["postgres"]' \
  "$container_name" "$image_ref" > /dev/null

if [[ "$push_image" == true ]]; then
  docker push "$image_ref"
fi

echo "$image_ref"
