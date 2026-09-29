#!/usr/bin/env bash
set -euo pipefail

ENTITIES_DIR="$(dirname "$0")/../crates/gateways/postgres/src/entities"

sea-orm-cli generate entity \
  -u "${DATABASE_URL}" \
  -o "$ENTITIES_DIR" \
  --with-serde both \
  --date-time-crate chrono
