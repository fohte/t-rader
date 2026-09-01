#!/usr/bin/env bash
set -euo pipefail

ENTITIES_DIR="$(dirname "$0")/../src/entities"

sea-orm-cli generate entity \
  -u "${DATABASE_URL}" \
  -o "$ENTITIES_DIR" \
  --with-serde both \
  --date-time-crate chrono \
  --model-extra-derives 'utoipa::ToSchema' \
  --enum-extra-derives 'utoipa::ToSchema'

# utoipa の ToSchema はデフォルトで struct 名をスキーマ名にするが、entity の struct は
# 全て `Model` という名前で衝突する。ファイル名から PascalCase 名を導出し
# `#[schema(as = ...)]` で個別に上書きする (英語の不規則複数形は例外テーブルで対応)。
declare -A SCHEMA_NAME_OVERRIDES=(
  [watchlists]=Watchlist
  [bars]=Bar
  [watchlist_items]=WatchlistItem
)

for file in "$ENTITIES_DIR"/*.rs; do
  base="$(basename "$file" .rs)"
  name="${SCHEMA_NAME_OVERRIDES[$base]:-}"
  if [[ -z "$name" ]]; then
    name="$(perl -pe 's/(^|_)([a-z])/\U$2/g' <<< "$base")"
  fi
  perl -i -pe "s/^pub struct Model \{\$/#[schema(as = $name)]\npub struct Model {/" "$file"
done

# note.graphs_json の実体は Vec<GraphDef> の JSON なので、entity の生 Json 型を上書きする
perl -i -pe 's/^    pub graphs_json: Json,$/    #[schema(value_type = Vec<crate::services::graph::GraphDef>)]\n    pub graphs_json: Json,/' \
  "$ENTITIES_DIR/note.rs"

# sea_orm の `DateTimeWithTimeZone` は `chrono::DateTime<FixedOffset>` の type alias。
# utoipa の chrono feature は型名を構文的に "DateTime" と照合するため、alias 名のままだと
# ToSchema/PartialSchema が実装されずコンパイルエラーになる。value_type で chrono::DateTime
# を直接指定して回避する。
for file in "$ENTITIES_DIR"/*.rs; do
  perl -i -pe 's/^(\s*)pub (\w+): DateTimeWithTimeZone,$/$1#[schema(value_type = chrono::DateTime<chrono::Utc>)]\n$1pub $2: DateTimeWithTimeZone,/' "$file"
  perl -i -pe 's/^(\s*)pub (\w+): Option<DateTimeWithTimeZone>,$/$1#[schema(value_type = Option<chrono::DateTime<chrono::Utc>>)]\n$1pub $2: Option<DateTimeWithTimeZone>,/' "$file"
done
