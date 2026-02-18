# CLAUDE.md

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
- `DATABASE_URL` のデフォルト値: `postgres://t_rader:t_rader@localhost:5432/t_rader_development`

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
