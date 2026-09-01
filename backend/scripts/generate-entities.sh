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

# 指定した行 (完全一致) の直前に1行挿入する。sed/perl は macOS (BSD sed / 古い perl) と
# CI の Linux (GNU sed / 別バージョンの perl) で実装が異なるため使わず、bash 組み込みのみで行う。
insert_before() {
  local file="$1" target="$2" insert="$3"
  local line result=""
  while IFS= read -r line || [ -n "$line" ]; do
    if [ "$line" = "$target" ]; then
      result+="$insert"$'\n'
    fi
    result+="$line"$'\n'
  done < "$file"
  printf '%s' "$result" > "$file"
}

# snake_case のファイル名を PascalCase に変換する (例: news_strategy_link -> NewsStrategyLink)
pascal_case() {
  local base="$1" part first name=""
  for part in ${base//_/ }; do
    first="$(printf '%s' "${part:0:1}" | tr '[:lower:]' '[:upper:]')"
    name+="${first}${part:1}"
  done
  printf '%s' "$name"
}

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
    *) name="$(pascal_case "$base")" ;;
  esac
  insert_before "$file" "pub struct Model {" "#[schema(as = $name)]"
done

# note.graphs_json の実体は Vec<GraphDef> の JSON なので、entity の生 Json 型を上書きする
insert_before "$ENTITIES_DIR/note.rs" "    pub graphs_json: Json," \
  "    #[schema(value_type = Vec<crate::services::graph::GraphDef>)]"

# utoipa は DateTimeWithTimeZone という型 alias 名を認識できないため、value_type で実型を指定する
for file in "$ENTITIES_DIR"/*.rs; do
  dt_lines=""
  opt_dt_lines=""
  while IFS= read -r line || [ -n "$line" ]; do
    case "$line" in
      *": DateTimeWithTimeZone,") dt_lines+="$line"$'\n' ;;
      *": Option<DateTimeWithTimeZone>,") opt_dt_lines+="$line"$'\n' ;;
    esac
  done < "$file"

  while IFS= read -r target; do
    [ -n "$target" ] || continue
    insert_before "$file" "$target" "    #[schema(value_type = chrono::DateTime<chrono::Utc>)]"
  done <<< "$dt_lines"

  while IFS= read -r target; do
    [ -n "$target" ] || continue
    insert_before "$file" "$target" "    #[schema(value_type = Option<chrono::DateTime<chrono::Utc>>)]"
  done <<< "$opt_dt_lines"
done
