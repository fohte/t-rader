#!/usr/bin/env bash
set -euo pipefail

# backend/openapi.json から TypeScript 型定義を生成する。
# Rust ビルドは不要 (openapi.json は git 追跡済み)。

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"

bunx openapi-typescript "${REPO_ROOT}/backend/openapi.json" \
  -o "${REPO_ROOT}/frontend/src/lib/api/schema.gen.d.ts"
