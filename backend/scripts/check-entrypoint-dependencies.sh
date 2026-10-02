#!/usr/bin/env bash
set -euo pipefail

backend_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
metadata=$(cargo metadata --manifest-path "$backend_root/Cargo.toml" --format-version 1 --no-deps)
violations=$(jq -r '
    . as $metadata
    | ($metadata.packages
        | map(select(.id as $id | ($metadata.workspace_members | index($id)) != null) | .name)
      ) as $workspace_packages
    | $metadata.packages[]
    | select(.manifest_path | contains("/crates/entrypoints/"))
    | . as $entrypoint
    | $entrypoint.dependencies[]
    | select(
        .name == "sea-orm"
        or (
          (.name as $name | $workspace_packages | index($name)) != null
          and .name != "core-application"
          and .name != "core-domain"
        )
      )
    | "\($entrypoint.name) -> \(.name) (\(.kind // "normal"))"
' <<< "$metadata")

if [[ -n $violations ]]; then
  printf 'entrypoint crate の禁止依存が見つかりました:\n%s\n' "$violations" >&2
  exit 1
fi

printf 'entrypoint crate の依存境界を確認しました。\n'
