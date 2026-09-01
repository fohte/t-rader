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

# macOS (BSD) と Linux (GNU) の sed/perl 互換性差分を回避するため bash 組み込みで処理する
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

pascal_case() {
  local base="$1" part first name=""
  for part in ${base//_/ }; do
    first="$(printf '%s' "${part:0:1}" | tr '[:lower:]' '[:upper:]')"
    name+="${first}${part:1}"
  done
  printf '%s' "$name"
}

# lines に列挙した各行の直前に insert を挿入する (改行区切り、空行は無視)
apply_overrides() {
  local file="$1" lines="$2" insert="$3" target
  while IFS= read -r target; do
    [ -n "$target" ] || continue
    insert_before "$file" "$target" "$insert"
  done <<< "$lines"
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

  apply_overrides "$file" "$dt_lines" "    #[schema(value_type = chrono::DateTime<chrono::Utc>)]"
  apply_overrides "$file" "$opt_dt_lines" "    #[schema(value_type = Option<chrono::DateTime<chrono::Utc>>)]"
done
