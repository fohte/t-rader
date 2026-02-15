# CLAUDE.md

## Bash commands

```bash
# DB 起動
docker compose up db -d

# バックエンド (ローカル)
cd backend && cargo run    # DATABASE_URL は backend/.env から読み込まれる
cd backend && cargo test
cd backend && cargo clippy -- -D warnings

# マイグレーション追加
cd backend && cargo sqlx migrate add <name>
```

## Core files

- `backend/migrations/` - sqlx マイグレーションファイル (起動時に自動実行)
- `backend/src/main.rs` - Axum サーバーのエントリポイント、DB 接続プール初期化
- `backend/src/error.rs` - AppError 型定義

## Warnings

- `backend/.env` は git 管理外。ローカル開発用の `DATABASE_URL` を含む
- CI / Docker ビルドでは `SQLX_OFFLINE=true` が必要 (`sqlx::query!` マクロ使用時)
- clippy で `unwrap_used`, `expect_used`, `panic` が deny。本番コードでは `?` と `map_err` を使うこと

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
