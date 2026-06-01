# CLAUDE.md

## Product

個人向け日本株投資プラットフォーム。

中心概念は「戦略」(= 永続ワークスペース)。長期投資 / 中期投資 / 集中スイング等を並列に運用し、戦略ごとに LLM がアナリスト役として自律的にノートとアノテーションを産出する。ユーザーは事後レビュー側に立ち、各アーティファクトには status (approved / unread / rejected)、コメントスレッド、変更履歴が紐づく。

リアルタイム性は重視せず、pull 型で「開いて読む」運用。通知サブシステムは永続的に持たない。

### 産出物の型はコードレベルでは 2 種のみ

個別戦略の中身 (どんなセマンティック分類でノートを書いているか等) を public リポジトリに含めない方針を取る。そのためセマンティック型 (observation / signal / thesis / hypothesis 等) をソースコードにハードコードしない。コードが持つ物理コンテナは次の 2 種のみ:

- **Markdown ノート**: 自由テキスト
- **Annotation**: text + 構造データ (対象パネル、位置、種別キー等)

セマンティック分類はユーザー / LLM が DB 上のタグ / frontmatter で表現する。enum / テーブル / API レスポンス型に observation や signal 等のラベルを直接定義しないこと。

### 一級参照型は 4 種、umbrella なし

ノートや分析カードから参照される一級型はこの 4 種のみ。それぞれ独立した id 体系で別テーブルにする (umbrella エンティティを作らない):

| kind        | 例                     |
| ----------- | ---------------------- |
| `stock`     | 7203                   |
| `indicator` | USDJPY, VIX            |
| `sector`    | 半導体                 |
| `theme`     | 円安, 米利上げサイクル |

横断検索は UNION クエリで対応する。

### Markdown 内リンクは prefix 必須

ノート / コメント / アノテーションの markdown 本文で参照型を指す内部リンクは prefix 付きにすること。prefix なしの `[[7203]]` は許容しない。

```text
[[stock:7203]]
[[indicator:USDJPY]]
[[sector:semiconductor]]
[[theme:weak-jpy]]
```

### 既存実装との関係

ローソク足チャート (Lightweight Charts)、ウォッチリスト、データプロバイダー抽象化は既存資産。ウォッチリストは MVP では残置するが、戦略 (= ワークスペース) ベースに移行後に deprecate 予定。新規 UI は戦略起点で組む。

設計の全体像と決定事項 (LLM ランタイム、MCP tool 設計、データソース段階導入、サンドボックス方針、取引履歴、リポジトリ戦略等) はリポ外で別途管理している。コード変更や PR でその文脈が必要な場合はユーザーに参照を求めること。

## Bash commands

```bash
# DB 起動 (全 worktree 共有、1 回だけ起動すればよい)
docker compose -f docker-compose.infra.yml up -d

# バックエンド (ローカル)
cd backend && cargo run
cd backend && cargo test
cd backend && cargo clippy -- -D warnings

# マイグレーション追加
cd backend/migration && cargo run -- generate <name>
# 生成後、lib.rs の Migrator::migrations() にも登録すること

# エンティティ再生成 (マイグレーション変更後に実行)
DATABASE_URL=... bash backend/scripts/generate-entities.sh

# フロントエンド
cd frontend && nr dev             # Vite 開発サーバー
cd frontend && nr test            # 型チェック + unit テスト
cd frontend && nr storybook       # Storybook 開発サーバー (http://localhost:6006)
cd frontend && nr storybook:build # Storybook 静的ビルド
```

## Core files

- `backend/migration/` - SeaORM マイグレーション crate (MigrationTrait で Rust ファイル、起動時に自動実行)
- `backend/src/entities/` - SeaORM Entity 定義 (`sea-orm-cli generate entity` で自動生成、手動編集禁止)
- `backend/scripts/generate-entities.sh` - エンティティ生成スクリプト (CLI オプション一元管理)
- `backend/src/main.rs` - Axum サーバーのエントリポイント、SeaORM DatabaseConnection 初期化
- `backend/src/error.rs` - AppError 型定義

## Migrations

- マイグレーションファイルは手動で作成しない。必ず `cd backend/migration && cargo run -- generate <name>` でファイルを生成してから up/down を実装すること
- ファイル名のタイムスタンプは CLI が自動付与する。`DeriveMigrationName` でファイル名からマイグレーション名を自動導出する
- 生成後、`backend/migration/src/lib.rs` の `Migrator::migrations()` に登録すること
- SeaQuery DSL でテーブル操作を記述するが、TimescaleDB 固有の SQL は `execute_unprepared` で raw SQL を使う
- 初期スキーマなど論理的にまとまる変更は 1 ファイルにまとめる。不必要にファイルを分割しない

## Entities

- `backend/src/entities/` 配下のファイルは `sea-orm-cli generate entity` で自動生成される。**手動編集禁止**
- スキーマ変更後は `bash backend/scripts/generate-entities.sh` を実行して再生成し、差分をコミットすること
- CI の `check-entity-sync` ジョブで DB スキーマとエンティティの整合性を自動検証する
- カスタムコード (将来的な `ActiveModelBehavior` 等) が必要な場合は `*_ext.rs` に分離すること

## 環境変数

- `.env` (git 管理) にローカル開発用のデフォルト値を定義している
- `.env.local` (git 管理外) で個人の環境に合わせた上書きが可能
- `.mise.toml` の `[env]` セクションで `.env` → `.env.local` の順に自動読み込みされる (mise が有効な環境では環境変数が自動で設定される)
- `DATABASE_URL` のデフォルト値は `.env` ファイルに定義されている

## DB 接続

- DB は `docker compose -f docker-compose.infra.yml up -d` で起動する (全 worktree 共有)
- `DATABASE_URL` は mise 経由で `.env` から自動的に読み込まれるため、手動設定は不要

## Warnings

- SeaORM は実行時に SQL を構築するため、Docker ビルド時の DB 接続は不要 (旧 `SQLX_OFFLINE` は廃止済み)
- clippy で `unwrap_used`, `expect_used`, `panic` が deny。本番コードでは `?` と `map_err` を使うこと

## Storybook

- フロントエンドの UI コンポーネントを作成・変更した際は、対応する Story ファイル (`*.stories.tsx`) も作成・更新すること
- Story ファイルはコンポーネントと同じディレクトリに配置する (例: `src/components/ui/button.stories.tsx`)
- TanStack Router に依存するコンポーネントは `createMemoryHistory` + `createRouter` + `RouterProvider` でルーターコンテキストを提供する

## Test code rules

### Parameterize similar test cases with rstest

Do not write multiple test functions that differ only in input/expected values. Use `#[rstest]` with `#[case]`.

```rust
// bad: separate functions per case
#[test]
fn test_parse_empty() { assert_eq!(parse(""), None); }
#[test]
fn test_parse_valid() { assert_eq!(parse("hello"), Some("hello")); }

// good: parameterized
#[rstest]
#[case::empty("", None)]
#[case::valid("hello", Some("hello"))]
fn test_parse(#[case] input: &str, #[case] expected: Option<&str>) {
    assert_eq!(parse(input), expected);
}
```

### Always name `#[case]` variants

Use `#[case::descriptive_name(...)]`, not bare `#[case(...)]`. Named cases identify failures without inspecting values.

### Use `#[fixture]` for shared test setup

Do not repeat the same setup code across tests. Extract into `#[fixture]`.

```rust
// bad: duplicated setup
#[rstest]
fn test_a() { let repo = make_repo(); /* ... */ }
#[rstest]
fn test_b() { let repo = make_repo(); /* ... */ }

// good: fixture injection
#[fixture]
fn repo() -> Repo { make_repo() }
#[rstest]
fn test_a(repo: Repo) { /* ... */ }
```

### Use `indoc!` for multiline string literals in tests

Do not embed `\n` in string literals. Use `indoc!` for readability.

### Extract repeated assertions into helper functions

If the same assertion chain appears in 3+ tests, extract it into a helper.

### Do not write tests that only verify test helpers

Tests must verify production code. Tests that only assert on test helpers, fixtures, or mocks are unnecessary. Remove them.
