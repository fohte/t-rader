# AGENTS.md

LLM / コーディングエージェントがリポジトリを変更するときに守るべき設計上のルール。実装規約 (ビルド/テスト/マイグレーション手順等) は `CLAUDE.md` を参照すること。

## 産出物の型はコードレベルでは 2 種のみ

戦略非公開ポリシーから、セマンティック型 (observation / signal / thesis / hypothesis 等) をソースコードにハードコードしないこと。コードが持つ物理コンテナは次の 2 種のみ:

- **Markdown ノート**: 自由テキスト
- **Annotation**: text + 構造データ (対象パネル、位置、種別キー等)

セマンティック分類はユーザー / LLM が DB 上のタグ / frontmatter で表現する。enum / テーブル / API レスポンス型に observation や signal 等のラベルを直接定義しないこと。

## 一級参照型は 4 種、umbrella なし

ノートや分析カードから参照される一級型はこの 4 種のみ。それぞれ独立した id 体系で別テーブルにする (umbrella エンティティを作らない):

| kind        | 例                     |
| ----------- | ---------------------- |
| `stock`     | 7203                   |
| `indicator` | USDJPY, VIX            |
| `sector`    | 半導体                 |
| `theme`     | 円安, 米利上げサイクル |

横断検索は UNION クエリで対応する。

## Markdown 内リンクは prefix 必須

ノート / コメント / アノテーションの markdown 本文で参照型を指す内部リンクは prefix 付きにすること。prefix なしの `[[7203]]` を許容しない。

```text
[[stock:7203]]
[[indicator:USDJPY]]
[[sector:semiconductor]]
[[theme:weak-jpy]]
```

パーサ / バリデータ / 自動補完を実装する際もこの形式を前提にすること。
