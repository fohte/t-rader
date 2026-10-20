# 変更履歴の保存形式

通常の `diff_json` は変更ごとに異なる JSON を保存する。定義全体を保存する履歴では `definition_snapshot` 形式を使う。

## definition_snapshot version 1

次のキーを持つ JSON object を保存する。

| キー      | 内容                                           |
| --------- | ---------------------------------------------- |
| `format`  | 固定値 `definition_snapshot`                   |
| `version` | 保存形式のバージョン。定義の revision ではない |
| `before`  | 変更前の定義全体。作成時は `null`              |
| `after`   | 変更後の定義全体。明示的な削除時は `null`      |

```json
{
  "format": "definition_snapshot",
  "version": 1,
  "before": { "example": "before" },
  "after": { "example": "after" }
}
```

作成では `before` を `null` にし、更新では前後の定義全体を保存する。明示的な削除では `after` を `null` にする。空文字列と `null` を区別し、部分差分ではなく各時点の定義を保存する。

この形式の履歴を書く実装では、現在の定義を読み取ってから定義と履歴を同じ transaction で確定する。並行更新時に `before` が直前の定義を表すよう、対象行をロックして読み取る。

この形式を導入する前の `diff_json` は変更せず、補完もしない。形式を読み取る側は `format` と `version` で判別する。
