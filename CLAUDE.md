# CLAUDE.md

## Product

fohte 個人用の日本株投資プラットフォーム。

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
# DB 起動 (全 worktree 共有、1 回だけ起動すればよい)。実ポートを反映した .env.runtime も生成される
mise run db-up

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

# エージェント (A2A server)
cd agent && nr dev  # tsx watch でローカル直接起動
cd agent && nr test # 型チェック + unit テスト (DB 統合テストは TEST_DATABASE_URL 未設定時は自動 skip)
```

## Core files

- `backend/migration/` - SeaORM マイグレーション crate (MigrationTrait で Rust ファイル、起動時に自動実行)
- `backend/src/entities/` - SeaORM Entity 定義 (`sea-orm-cli generate entity` で自動生成、手動編集禁止)
- `backend/scripts/generate-entities.sh` - エンティティ生成スクリプト (CLI オプション一元管理)
- `backend/src/main.rs` - Axum サーバーのエントリポイント、SeaORM DatabaseConnection 初期化
- `backend/src/error.rs` - AppError 型定義
- `backend/src/agent_client/` - t-rader-agent 内部 API client (`AgentTaskClient` trait、戦略タスクの投入 / 状態照会)
- `backend/src/services/strategy_tasks.rs` - 戦略タスク投入の共通 service (`submit_task`、5 経路から呼ばれる)
- `backend/src/mcp/watcher.rs` - 戦略タスクの phase polling (pending/running 行の状態照会 + deadline 超過の失敗確定)
- `backend/src/handlers/agent_tasks.rs` - t-rader-agent からのタスク決着 webhook 受信
- `agent/src/main.ts` - A2A server のエントリポイント、Hono app の組み立て
- `agent/src/a2a/executor.ts` - `TraderAgentExecutor` (`strategy_id` metadata があれば即実行、なければ `agent/src/strategy-resolution/` で名前解決した上で `agent/src/strategy-agent/` の `runStrategyAgent` に委譲)
- `agent/src/strategy-agent/strategy-agent.ts` - agent-config 取得 + LangGraph agent 構成 + MCP tool 呼び出しの実行ロジック
- `agent/src/strategy-resolution/resolve-strategy.ts` - 戦略候補一覧から自由文の対象戦略を決定的な文字列類似度で解決するロジック
- `agent/src/internal-api/routes.ts` - backend 向け internal API (`POST /internal/tasks`, `GET /internal/tasks/{task_id}`)
- `agent/drizzle/` - drizzle-orm マイグレーション (起動時に自動実行)

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
- `.env.runtime` (git 管理外、`mise run db-up` が自動生成) で DB のランダム割り当てポートなど、起動するたびに変わる値を定義する
- `.env.local` (git 管理外) で個人の環境に合わせた上書きが可能
- `.mise.toml` の `[env]` セクションで `.env` → `.env.runtime` → `.env.local` の順に自動読み込みされる (mise が有効な環境では環境変数が自動で設定される)。`.env.local` が常に最後に勝つ

## DB 接続

- DB は `mise run db-up` で起動する (`docker compose -f docker-compose.infra.yml up -d --wait` のラッパー、全 worktree 共有)
- db のホストポートはランダム割り当てのため、`mise run db-up` が実ポートを反映した `DATABASE_URL` を `.env.runtime` に書き出す。手動でのポート確認は不要になる
- `cargo run` 等でローカル直接起動する場合、`.env.runtime` の値がそのまま使われる
- agent をローカル直接起動する場合は、agent 専用の論理 DB (`t_rader_agent_development`) を指す `DATABASE_URL` を `.env.local` で上書きすること (`.env.runtime` の値は backend 用)

## Warnings

- SeaORM は実行時に SQL を構築するため、Docker ビルド時の DB 接続は不要 (旧 `SQLX_OFFLINE` は廃止済み)
- clippy で `unwrap_used`, `expect_used`, `panic` が deny。本番コードでは `?` と `map_err` を使うこと

## Storybook

- フロントエンドの UI コンポーネントを作成・変更した際は、対応する Story ファイル (`*.stories.tsx`) も作成・更新すること
- Story ファイルはコンポーネントと同じディレクトリに配置する (例: `src/components/ui/button.stories.tsx`)
- TanStack Router に依存するコンポーネントは `createMemoryHistory` + `createRouter` + `RouterProvider` でルーターコンテキストを提供する

## Code organization rules

### Split files before they grow past ~500 lines of production code

When a change would push a file's non-test code past ~500 lines, split it along responsibility seams before adding more. Splits must be move-only commits: no logic changes, renames, or reformatting mixed in. Keep external import paths unchanged by keeping the entrypoint file in place and re-exporting the pieces you split out into new files (e.g. `foo.rs` gains a `foo/` directory for its submodules, `index.ts` re-exports from the new files). Tests move together with the code they verify.

Prefer creating a new focused file over appending to the largest existing one.

## Error handling rules

### Return a `Result` instead of throwing

`errorHandling` in `eslint.config.js` bans `throw`/`try-catch` in production code and requires every returned `Result` to be consumed (`no-restricted-syntax`, `neverthrow/must-use-result` in `@fohte/eslint-config`). Return a `Result`/`ResultAsync` from [neverthrow](https://github.com/supermacro/neverthrow) instead:

```ts
// bad: throws
function parseConfig(raw: string): Config {
  if (!isValid(raw)) throw new Error('invalid config')
  return JSON.parse(raw)
}

// good: returns a Result
function parseConfig(raw: string): Result<Config, ConfigError> {
  if (!isValid(raw)) return err(new ConfigError('invalid config'))
  return ok(JSON.parse(raw))
}
```

Use `ResultAsync.fromPromise()` or `Result.fromThrowable()` to interop with a throwing API without a local try/catch. If the throw-based contract genuinely can't be wrapped that way, catch the exception, wrap it in a `BoundaryError` subclass (see `src/errors.ts`), and rethrow it — `no-restricted-syntax` bans `try`/`throw` as separate selectors, so both the `try` and the `throw` need their own `eslint-disable-next-line no-restricted-syntax` comment explaining why.

## Test code rules

### Assert on the whole output with a single equality check

Treat each test as a spec: build the expected output as one literal value (object, struct, JSON, array, etc.) and compare it to the actual output with a single equality assertion. Do not split the assertion into per-field checks, and do not use partial matchers (substring contains, `toContain`, `toMatchObject`, prefix/suffix checks, regex-on-substring, etc.). Partial matches silently ignore unexpected fields and extra elements, so the test stops working as a spec the moment the shape of the output changes.

```ts
// bad: picks fields one by one — silent on any new/changed field
const ev = run()
expect(ev.path).toBe('/a')
expect(ev.event).toBe('ok')
expect(ev.message).toContain('done')

// good: one literal, one equality — any drift in shape fails the test
expect(run()).toEqual({
  path: '/a',
  event: 'ok',
  message: 'done',
})
```

```rust
// bad
let ev = run();
assert_eq!(ev["path"], "/a");
assert_eq!(ev["event"], "ok");
assert!(ev["message"].as_str().unwrap().contains("done"));

// good
assert_eq!(
    run(),
    json!({
        "path": "/a",
        "event": "ok",
        "message": "done",
    }),
);
```

For dynamic fields (timestamps, UUIDs, random IDs), normalize them in a helper before the comparison (e.g. replace with a fixed placeholder) so the full output can still be asserted in one equality check. Do not weaken the assertion to dodge the dynamic value.

The `no-assert-contains` ast-grep rule rejects `assert!(x.contains(...))` at the expression level; this guideline is the broader principle that the rule is one instance of.

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
