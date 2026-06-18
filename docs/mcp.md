# MCP server

t-rader-backend (Axum) 内に 2 つの MCP server (`rmcp` ベースの Streamable HTTP) を同居させ、利用者ごとに別 path で露出する。

| path            | 用途                                                                                                  | 主な利用者                            |
| --------------- | ----------------------------------------------------------------------------------------------------- | ------------------------------------- |
| `/mcp/mgmt`     | 管理 MCP。戦略状態の参照と戦略タスク投入                                                              | personal-bot 等のコントロールプレーン |
| `/mcp/strategy` | 戦略実行 MCP。戦略 Agent が戦略境界内のリソース (ノート / アノテーション / 価格 / Python 実行) を操作 | 戦略 Agent (kubeopencode 上)          |

両 path とも JSON-RPC over Streamable HTTP (SSE) を喋る。クライアントは `initialize` → `mcp-session-id` ヘッダで以後のリクエストを継続する。

## 管理 MCP (`/mcp/mgmt`)

personal-bot 等から呼ばれる。tool 単位の認可は持たず、Tailscale VPN 境界と Cloudflare Access による前段で担保する。

| tool                       | 入力                                    | 出力 (要約)                                                                                |
| -------------------------- | --------------------------------------- | ------------------------------------------------------------------------------------------ |
| `list_strategies`          | (なし)                                  | 戦略一覧 (`strategy_id`, `name`, `updated_at`, `unread_card_count`)                        |
| `submit_strategy_task`     | `strategy_id`, `prompt`                 | `task_id`, `kubeopencode_task_name`。DB に `strategy_task` 行を作り Task CR を投入する     |
| `get_strategy_task_status` | `kubeopencode_task_name`                | `phase` (`pending` / `running` / `completed` / `failed`), `error_summary`, `updated_at` 等 |
| `list_recent_notes`        | `strategy_id`, `limit` (任意、最大 100) | 最新ノートのメタデータ一覧                                                                 |
| `list_recent_annotations`  | `strategy_id`, `limit` (任意、最大 100) | 最新アノテーションのメタデータ一覧                                                         |

`submit_strategy_task` は kubeopencode API クライアント経由で `kubeopencode` namespace に Task CR を作成する。詳細は [`docs/kubeopencode-integration.md`](./kubeopencode-integration.md) を参照。

## 戦略実行 MCP (`/mcp/strategy`)

戦略 Agent Pod (1 戦略 = 1 Agent) からのみ呼ばれる想定。NetworkPolicy で `kubeopencode` namespace 内 Pod に接続元を限定する。

### 戦略境界の保証

戦略 Agent は接続時に `x-strategy-id` HTTP ヘッダで自身の `strategy_id` を持ち込む。

- ヘッダが欠落 / 非 UUID なら MCP 層で reject する。
- tool 引数の `strategy_id` がヘッダの値と一致しない呼び出しも reject する。
- 対象リソース (note / annotation) の `strategy_id` も Repository 層で二重検査し、戦略 A の Agent が戦略 B のリソースに触れないことを保証する。

### tool 一覧

| tool                | 入力                                            | 出力 (要約)                                                                         |
| ------------------- | ----------------------------------------------- | ----------------------------------------------------------------------------------- |
| `query_data`        | `strategy_id`, `symbol`, 期間                   | 価格時系列 (内部で DataProvider 抽象を経由)                                         |
| `write_note`        | `strategy_id`, `note_id?`, `title`, `body`, ... | ノートを作成または更新。`actor` は LLM 固定 (`"llm"`)                               |
| `read_note`         | `strategy_id`, `note_id`                        | ノート本文とメタデータ                                                              |
| `list_notes`        | `strategy_id`, `limit`                          | ノート一覧 (デフォルト 50、最大 200)                                                |
| `create_annotation` | `strategy_id`, `kind`, `target`, `body`, ...    | アノテーション作成。`kind` は `signal` / `level` / `observation` / `other`          |
| `read_annotations`  | `strategy_id`, `limit`                          | アノテーション一覧                                                                  |
| `eval_python`       | `strategy_id`, `code`, `stdin?`, `timeout?`     | Python コードを exec Pod (Kata Containers) 上で実行。stdout/stderr/exit code を返す |

`eval_python` は入力値に対する純粋関数評価モデル。exec Pod は 1 container / 単一 Python プロセス / root filesystem read-only / subprocess 不可 / network 全 deny。入出力は stdin → stdout/stderr/exit code のみ。

## session 永続化

`mcp-session-id` ヘッダで識別される session は PostgreSQL の `mcp_session_state` テーブルに永続化される (`PostgresSessionStore`)。

- `initialize` 時のハンドシェイクパラメータが DB に保存される。
- backend Pod が再起動しても、同じ `mcp-session-id` を送れば `StreamableHttpService` が state を読み出して in-memory session を再構築し、initialize を replay する。これにより数日スパンの long-lived session が backend rolling restart を跨いで継続できる。
- session TTL は既定 7 日。`updated_at` が TTL を超えた行はバックグラウンド GC タスクで削除する (既定間隔 1 時間)。
- session 解決時の inline GC は best-effort で、失敗しても未知 session として扱い新規 initialize を案内する。

## `MCP_ALLOWED_HOSTS`

`rmcp` の Streamable HTTP transport は DNS rebinding 対策として `Host` ヘッダの allowlist を持つ。既定値は `localhost` / `127.0.0.1` / `::1` のみで、Kubernetes Service DNS (例: `t-rader-backend.t-rader.svc.cluster.local`) 経由のアクセスは 403 で弾かれる。

`MCP_ALLOWED_HOSTS` 環境変数 (カンマ区切り) で allowlist を拡張する。

```
MCP_ALLOWED_HOSTS=t-rader-backend.t-rader.svc.cluster.local,other-host.example.com
```

- port を含めずホスト名だけを書けば任意 port を許容する。
- 空白は trim し、空エントリは無視する。
- in-cluster 配備時は backend Service の DNS 名を必ず追加すること。追加しないと personal-bot / 戦略 Agent からの接続が全て 403 で弾かれる。

具体的な配備時の env / RBAC は [`docs/deployment.md`](./deployment.md) を参照。
