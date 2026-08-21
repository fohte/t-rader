# MCP server

t-rader-backend (Axum) 内に 2 つの MCP server (`rmcp` ベースの Streamable HTTP) を同居させ、利用者ごとに別 path で露出する。

| path            | 用途                                                                                                  | 主な利用者                                                                             |
| --------------- | ----------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------- |
| `/mcp/mgmt`     | 管理 MCP。戦略状態の参照と戦略タスク投入                                                              | 外部のコントロールプレーンクライアント、t-rader-agent (戦略解決時の `list_strategies`) |
| `/mcp/strategy` | 戦略実行 MCP。戦略 Agent が戦略境界内のリソース (ノート / アノテーション / 価格 / Python 実行) を操作 | t-rader-agent                                                                          |

両 path とも JSON-RPC over Streamable HTTP (SSE) で通信する。`rmcp` は MCP spec [2026-07-28](https://modelcontextprotocol.io/specification/2026-07-28) までのプロトコル世代を自動判別して同時に扱う。

- **`< 2026-07-28` (レガシー)**: `initialize` → `mcp-session-id` ヘッダで以後のリクエストを継続する。t-rader-agent (`@modelcontextprotocol/sdk`) はこの世代で接続する。
- **`2026-07-28` 以降**: SEP-2567 により session の概念が無くなり、リクエストごとに stateless に処理される (`mcp-session-id` は発行されない)。discover lifecycle (SEP-2575) を使う外部クライアントはこの世代で接続する。`initialize` を経ず `MCP-Protocol-Version` ヘッダのみで直接 tool を呼ぶ場合、SEP-2243 の `Mcp-Method` ヘッダも必須になる (`Mcp-Name` は `tools/call` など対象を名指しする method でのみ必須)。

## 管理 MCP (`/mcp/mgmt`)

外部のコントロールプレーンクライアントから呼ばれる。tool 単位の認可は持たず、ネットワーク境界 (VPN / Zero Trust proxy 等) と前段認証で担保する想定。

| tool                       | 入力                                                                                                              | 出力 (要約)                                                                                                  |
| -------------------------- | ----------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------ |
| `list_strategies`          | (なし)                                                                                                            | 戦略一覧 (`strategy_id`, `name`, `updated_at`, `unread_card_count`)                                          |
| `submit_strategy_task`     | `strategy_id`, `prompt`                                                                                           | `task_id`, `a2a_task_id`。DB に `strategy_task` 行を作り t-rader-agent にタスクを投入する                    |
| `get_strategy_task_status` | `a2a_task_id`                                                                                                     | `phase` (`pending` / `running` / `completed` / `failed`), `error_summary`, `result_text`, `updated_at` 等    |
| `get_strategy_config`      | `strategy_id`                                                                                                     | 戦略設定一式 (`name`, `description`, `agents_md`, `skills`, `agent_graph`) と紐づく `triggers` 一覧          |
| `create_strategy`          | `name`, `description?`, `agents_md?`, `skills?`, `agent_graph?`                                                   | `ok`, `errors`, `strategy_id?`。検証エラーがあれば 1 件も書き込まず `ok=false` で全エラーを返す              |
| `update_strategy_config`   | `strategy_id`, `name?`, `description?`, `agents_md?`, `skills?` (JSON Merge Patch), `agent_graph?`                | `ok`, `errors`。指定フィールドのみ 1 回の呼び出しで atomic に更新する                                        |
| `delete_strategy`          | `strategy_id`, `confirm_name`                                                                                     | `ok`, `errors`。`confirm_name` が戦略名と完全一致しないと削除しない。一致すれば関連リソースごと cascade 削除 |
| `create_strategy_trigger`  | `strategy_id`, `kind` (`cron` / `hook`), `schedule?`, `hook_slug?`, `event_match?`, `prompt_template`, `enabled?` | `ok`, `errors`, `trigger_id?`。kind=cron は schedule 必須 (hook_slug 禁止)、kind=hook はその逆               |
| `update_strategy_trigger`  | `trigger_id`, `schedule?`, `hook_slug?`, `event_match?`, `prompt_template?`, `enabled?`                           | `ok`, `errors`。指定フィールドのみ更新。kind / strategy_id は不変                                            |
| `delete_strategy_trigger`  | `trigger_id`                                                                                                      | `ok`, `errors`                                                                                               |
| `list_recent_notes`        | `strategy_id`, `limit?`                                                                                           | 最新ノートのメタデータ一覧                                                                                   |
| `list_recent_annotations`  | `strategy_id`, `limit?`                                                                                           | 最新アノテーションのメタデータ一覧                                                                           |
| `list_rss_feeds`           | `enabled_only?`                                                                                                   | RSS フィード定義一覧                                                                                         |
| `create_rss_feed`          | `source`, `display_name`, `url`, `enabled?`                                                                       | 作成した RSS フィード定義                                                                                    |
| `update_rss_feed`          | `id`, `display_name?`, `url?`, `enabled?`                                                                         | 更新後の RSS フィード定義                                                                                    |
| `delete_rss_feed`          | `id`                                                                                                              | `id`。既存の `news_item` 行は残す                                                                            |

`submit_strategy_task` は t-rader-agent の内部 API (`POST /internal/tasks`) 経由でタスクを投入する。クライアント実装は `backend/src/agent_client/` が SSOT。投入から決着までの共通ロジックは `backend/src/services/strategy_tasks.rs`、決着 polling は `backend/src/mcp/watcher.rs` を参照。

`create_strategy` / `update_strategy_config` / `delete_strategy` による DB 書き込みは REST (`backend/src/handlers/strategies/mod.rs`) と共通の `backend/src/services/strategy_config.rs` を経由し、`change_history` には actor `llm` / label `mgmt-mcp` で記録される。

`create_strategy_trigger` / `update_strategy_trigger` / `delete_strategy_trigger` は REST (`POST /api/strategies/{id}/triggers`, `PUT /api/triggers/{trigger_id}`, `DELETE /api/triggers/{trigger_id}`) と共通の `backend/src/services/trigger_crud.rs` を経由する。`change_history.target_kind` の CHECK 制約は `"trigger"` を含まないため、trigger への書き込みは change_history に記録されない (既知の監査ギャップ)。

## 戦略実行 MCP (`/mcp/strategy`)

t-rader-agent からのみ呼ばれる想定。接続元の絞り込みはクラスタ側のネットワーク境界で組み立てる。

### 戦略境界の保証

t-rader-agent は接続時に `x-strategy-id` HTTP ヘッダで自身が実行中の `strategy_id` を持ち込む。

- ヘッダが欠落 / 非 UUID なら MCP 層で reject する。
- tool 引数の `strategy_id` がヘッダの値と一致しない呼び出しも reject する。
- 対象リソース (note / annotation) の `strategy_id` も Repository 層で二重検査し、戦略 A の Agent が戦略 B のリソースに触れないことを保証する。

### tool 一覧

| tool                | 入力                                                                                            | 出力 (要約)                                                                                                                                                                                                                                          |
| ------------------- | ----------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `query_data`        | `strategy_id`, `instrument_id`, `from` (YYYY-MM-DD), `to` (YYYY-MM-DD)                          | 価格時系列 (内部で DataProvider 抽象を経由)                                                                                                                                                                                                          |
| `write_note`        | `strategy_id`, `note_id?`, `title?`, `body_md?`, `type_tag?`, `frontmatter_json?`, `graphs?`    | ノートを作成または更新。`created_by_kind` はサーバー側で `"llm"` 固定。更新時は status を unread に戻す。`graphs` (`GraphDef` の配列、定義は `backend/src/services/graph.rs`) は配列ごと置換 (id 単位の部分更新は不可)。省略時は既存の図を変更しない |
| `read_note`         | `strategy_id`, `note_id`                                                                        | ノート本文 / メタデータ / `graphs` (`GraphDef` の配列)                                                                                                                                                                                               |
| `list_notes`        | `strategy_id`, `limit?`                                                                         | ノート一覧                                                                                                                                                                                                                                           |
| `create_annotation` | `strategy_id`, `target_symbol`, `target_kind`, `timestamp`, `text`, `price?`, `linked_note_id?` | アノテーション作成。`target_kind` は `signal` / `level` / `observation` / `other`                                                                                                                                                                    |
| `read_annotations`  | `strategy_id`, `target_symbol?`, `limit?`                                                       | アノテーション一覧 (任意で `target_symbol` によるフィルタが可能)                                                                                                                                                                                     |
| `read_comments`     | `strategy_id`, `target_kind` (`note` / `annotation`), `target_id`, `resolved?`                  | 対象に付いたレビューコメント一覧 (古い順)。スレッドは `parent_id` で表現。`resolved` で絞り込み可。各コメントは `anchor_text`/`start_line`/`end_line`/`drifted` を含み、ノート本文中の位置を追跡する                                                 |
| `resolve_comment`   | `strategy_id`, `comment_id`, `resolved`                                                         | レビューコメントを解決済み/未解決に切り替える                                                                                                                                                                                                        |
| `reply_comment`     | `strategy_id`, `parent_id`, `body`                                                              | レビューコメントに返信する。`author_kind` はサーバー側で `"llm"` 固定                                                                                                                                                                                |
| `eval_python`       | `strategy_id`, `code`, `stdin?`, `timeout_secs?`, `max_output_bytes?`                           | Python コードを exec Pod (Kata Containers) 上で実行。stdout/stderr/exit code を返す                                                                                                                                                                  |
| `add_interest`      | `strategy_id`, `ref_kind`, `ref_id`                                                             | 関心 (`role=derived`, `origin=llm`) を追加。既存と一致すれば idempotent に成功                                                                                                                                                                       |
| `eval_indicator`    | `strategy_id`, `name`, `args`, `timeout_secs?`, `max_output_bytes?`                             | 永続化された indicator を exec Pod 上で評価 (戦略 scope 優先、無ければ global)                                                                                                                                                                       |

`eval_python` は入力値に対する純粋関数評価モデルとして設計している。1 evaluation = 1 exec Pod で起動し、入出力は stdin → stdout/stderr/exit code のみ。Pod spec 側の隔離 (read-only rootfs、non-root、capabilities drop、deadline 等) は backend が固定する。namespace 側の隔離 (RuntimeClass `kata`、NetworkPolicy 全 deny、Pod Security Admission) は [`docs/deployment.md`](./deployment.md) を参照。

## session 永続化

`mcp-session-id` ヘッダで識別される session は PostgreSQL の `mcp_session_state` テーブルに永続化される (`PostgresSessionStore`)。stateless な `2026-07-28` 世代のリクエストは session を発行しないため、この永続化の対象外。

- `initialize` 時のハンドシェイクパラメータが DB に保存される。
- backend Pod が再起動しても、同じ `mcp-session-id` を送れば `StreamableHttpService` が state を読み出して in-memory session を再構築し、initialize を replay する。これにより数日スパンの long-lived session が backend rolling restart を跨いで継続できる。
- `updated_at` が TTL を超えた行はバックグラウンド GC タスクで定期的に削除する (TTL と GC 間隔の既定値は `backend/src/mcp/store.rs` 参照)。
- session 解決時の inline GC は best-effort で、失敗しても未知 session として扱い新規 initialize を案内する。

## `MCP_ALLOWED_HOSTS`

`rmcp` の Streamable HTTP transport は DNS rebinding 対策として `Host` ヘッダの allowlist を持つ。既定値は `localhost` / `127.0.0.1` / `::1` のみで、Kubernetes Service DNS 経由のアクセスは 403 で弾かれる。

`MCP_ALLOWED_HOSTS` 環境変数 (カンマ区切り) で allowlist を拡張する。

```
MCP_ALLOWED_HOSTS=<backend-service-fqdn>,<other-host>
```

- port を含めずホスト名だけを書けば任意 port を許容する。
- 空白は trim し、空エントリは無視する。
- in-cluster 配備時は backend Service の DNS 名を必ず追加すること。追加しないと外部の MCP クライアントからの接続が全て 403 で弾かれる。

具体的な配備時の env / RBAC は [`docs/deployment.md`](./deployment.md) を参照。
