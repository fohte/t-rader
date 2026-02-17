#!/usr/bin/env bash
set -euo pipefail

sea-orm-cli generate entity \
  -u "${DATABASE_URL}" \
  -o "$(dirname "$0")/../src/entities" \
  --with-serde both \
  --date-time-crate chrono
