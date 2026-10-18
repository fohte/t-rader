#!/usr/bin/env bash
set -euo pipefail

readonly BASE_IMAGE='timescale/timescaledb:2.26.4-pg18@sha256:9ecddf2c88356f01f2986a0349ea525a5f7c56b81935bbfd58e9590c93f7d859'
readonly IMAGE_REPOSITORY='ghcr.io/fohte/t-rader/perf-db'
readonly POSTGRES_USER='t_rader'
readonly POSTGRES_PASSWORD='t_rader'
readonly POSTGRES_DATABASE='t_rader'

push_image=false
if [[ $# -eq 1 && "$1" == '--push' ]]; then
  push_image=true
elif [[ $# -gt 0 ]]; then
  echo "Usage: $0 [--push]" >&2
  exit 2
fi

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd -- "$script_dir/../.." && pwd)"
seed_sql="$repo_root/backend/perf-db/seed.sql"

source_hash=$(
  {
    find "$repo_root/backend/migration" -type f -print
    printf '%s\n' "$seed_sql" "$script_dir/build-perf-db-image.sh"
  } |
    LC_ALL=C sort |
    while IFS= read -r path; do
      printf '%s %s\n' "${path#"$repo_root"/}" "$(git -C "$repo_root" hash-object -- "$path")"
    done |
    openssl dgst -sha256 |
    awk '{print $NF}'
)
image_ref="$IMAGE_REPOSITORY:$source_hash"
container_name="t-rader-perf-db-build-$$"
container_started=false

cleanup() {
  if [[ "$container_started" == true ]]; then
    docker stop --timeout 60 "$container_name" > /dev/null 2>&1 || true
    docker rm "$container_name" > /dev/null 2>&1 || true
  fi
}
trap cleanup EXIT

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
  -c max_parallel_workers=0 \
  -c max_parallel_workers_per_gather=0 \
  -c autovacuum_max_workers=2 \
  -c max_wal_size=1GB \
  -c min_wal_size=32MB > /dev/null
container_started=true

ready=false
for _ in {1..120}; do
  if docker exec "$container_name" psql \
    -h 127.0.0.1 \
    -U "$POSTGRES_USER" \
    -d "$POSTGRES_DATABASE" \
    -v ON_ERROR_STOP=1 \
    -c 'SELECT 1' > /dev/null 2>&1; then
    ready=true
    break
  fi
  sleep 1
done
if [[ "$ready" != true ]]; then
  echo 'PostgreSQL did not become ready within 120 seconds.' >&2
  docker logs "$container_name" >&2
  exit 1
fi

port="$(docker port "$container_name" 5432/tcp | sed 's/.*://')"
(
  cd "$repo_root/backend"
  DATABASE_URL="postgres://$POSTGRES_USER:$POSTGRES_PASSWORD@127.0.0.1:$port/$POSTGRES_DATABASE" \
    cargo run -p migration -- up
)

docker exec -i "$container_name" psql \
  -U "$POSTGRES_USER" \
  -d "$POSTGRES_DATABASE" \
  -v ON_ERROR_STOP=1 < "$seed_sql"

docker exec "$container_name" psql \
  -U "$POSTGRES_USER" \
  -d "$POSTGRES_DATABASE" \
  -v ON_ERROR_STOP=1 \
  -c 'CHECKPOINT' > /dev/null
docker stop --timeout 60 "$container_name" > /dev/null
docker commit \
  --change 'ENV PGDATA=/pgdata' \
  --change "ENV POSTGRES_USER=$POSTGRES_USER" \
  --change "ENV POSTGRES_PASSWORD=$POSTGRES_PASSWORD" \
  --change "ENV POSTGRES_DB=$POSTGRES_DATABASE" \
  --change 'CMD ["postgres"]' \
  "$container_name" "$image_ref" > /dev/null

if [[ "$push_image" == true ]]; then
  docker push "$image_ref"
fi

echo "$image_ref"
