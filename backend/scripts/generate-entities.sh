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
for file in "$ENTITIES_DIR"/*.rs; do
  base="$(basename "$file" .rs)"
  case "$base" in
    watchlists) name=Watchlist ;;
    bars) name=Bar ;;
    watchlist_items) name=WatchlistItem ;;
    instruments) name=Instrument ;;
    *) name="$(perl -pe 's/(^|_)([a-z])/\U$2/g' <<< "$base")" ;;
  esac
  perl -i -pe "s/^pub struct Model \{\$/#[schema(as = $name)]\npub struct Model {/" "$file"
done

# note.graphs_json の実体は Vec<GraphDef> の JSON なので、entity の生 Json 型を上書きする
perl -i -pe 's/^    pub graphs_json: Json,$/    #[schema(value_type = Vec<crate::services::graph::GraphDef>)]\n    pub graphs_json: Json,/' \
  "$ENTITIES_DIR/note.rs"

# utoipa は DateTimeWithTimeZone という型 alias 名を認識できないため、value_type で実型を指定する
for file in "$ENTITIES_DIR"/*.rs; do
  perl -i -pe 's/^(\s*)pub (\w+): DateTimeWithTimeZone,$/$1#[schema(value_type = chrono::DateTime<chrono::Utc>)]\n$1pub $2: DateTimeWithTimeZone,/' "$file"
  perl -i -pe 's/^(\s*)pub (\w+): Option<DateTimeWithTimeZone>,$/$1#[schema(value_type = Option<chrono::DateTime<chrono::Utc>>)]\n$1pub $2: Option<DateTimeWithTimeZone>,/' "$file"
done
