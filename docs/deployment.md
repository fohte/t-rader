# Deployment

t-rader-backend は Kubernetes クラスタ上に Deployment + Service として配備される想定。backend 自体は Axum プロセスを 1 つだけ起動し、LLM 実行は kubeopencode operator (別 namespace) に委譲する。

実際のクラスタ、namespace、Helm chart、外部公開経路は別途 infra リポジトリで管理する。

## Service と接続経路

- backend は port `3000` を listen する (`BACKEND_PORT` で変更可)。
- frontend Pod 内 nginx が `/api` を backend Service にリバースプロキシし、SPA から見て同一オリジン構成にする想定。
- MCP クライアント (戦略 Agent / 管理 MCP の外部コントロールプレーンクライアント) は in-cluster Service DNS で MCP path (`/mcp/strategy`, `/mcp/mgmt`) を叩く。`MCP_ALLOWED_HOSTS` に backend Service の DNS 名を追加しないと `rmcp` の DNS rebinding 保護で 403 になる ([`docs/mcp.md`](./mcp.md))。
- backend Service を直接 Ingress しない。外部公開は frontend のみで、その経路は infra リポジトリ側で組む。

## 環境変数

| 区分              | 変数                   | 意味                                                                            | 未設定時の挙動                                                                   |
| ----------------- | ---------------------- | ------------------------------------------------------------------------------- | -------------------------------------------------------------------------------- |
| 必須              | `DATABASE_URL`         | PostgreSQL 接続 URL。起動時にマイグレーションを自動実行する                     | 起動失敗                                                                         |
| 必須 (production) | `KUBEOPENCODE_API_URL` | kube-apiserver の base URL (in-cluster なら `https://kubernetes.default.svc`)   | warning ログを出して `DisabledKubeopencodeClient` で起動。Task CR 投入は全て失敗 |
| 必須 (in-cluster) | `MCP_ALLOWED_HOSTS`    | MCP server が受理する `Host` ヘッダの追加許可リスト (カンマ区切り)              | in-cluster Service DNS が 403 で弾かれる ([`docs/mcp.md`](./mcp.md))             |
| 必須 (production) | `KATA_EXEC_API_URL`    | Kata Containers exec Pod を起動するための kube-apiserver base URL               | warning ログを出して `eval_python` tool が disabled になる                       |
| 任意              | `DATA_PROVIDER`        | 価格データ取得元の選択 (`jquants` / `ibkr` / `none`)。未設定なら `jquants` 扱い | `jquants` で起動                                                                 |
| 条件付き          | `JQUANTS_API_KEY`      | `DATA_PROVIDER=jquants` 時のみ必要                                              | DataProvider なしで起動 (`query_data` が動かない)                                |
| 条件付き          | `IBKR_BASE_URL` 等     | `DATA_PROVIDER=ibkr` 時に Gateway 接続情報を設定                                | デフォルト値で初期化を試行                                                       |
| 任意              | `BACKEND_PORT`         | listen port                                                                     | `3000` で listen                                                                 |

kubeopencode / kata-exec の追加の任意変数 (`KUBEOPENCODE_NAMESPACE`, `KATA_EXEC_NAMESPACE`, `KATA_EXEC_IMAGE`, `KATA_EXEC_TOKEN`, `KATA_EXEC_DEFAULT_TIMEOUT_SECS`, `KATA_EXEC_MAX_OUTPUT_BYTES` 等) は backend のソース (`backend/src/kubeopencode/`, `backend/src/kata_exec/`) を参照。in-cluster で動かす場合 token / CA は ServiceAccount のものを自動で読む。

## backend が要求する権限

backend ServiceAccount が必要とする操作を、対象 namespace の役割ごとに列挙する。実際の RoleBinding / RBAC YAML は infra リポジトリで組み立てる。

### 戦略 Agent ランタイム namespace (kubeopencode operator が動く namespace)

戦略 Agent ライフサイクル (Agent CR と関連 ServiceAccount / ConfigMap / ExternalSecret の provisioning) と Task CR の投入・監視を backend が直接担う ([`docs/kubeopencode-integration.md`](./kubeopencode-integration.md))。必要な操作は以下を機能粒度で表す:

- Agent CR の CRUD と watch
- Task CR の作成と read (watch を含む)
- 戦略 Agent 用 ServiceAccount / ConfigMap / ExternalSecret の lifecycle 管理

具体的な resource 名 / API group / verb のセットは `backend/src/kubeopencode/` 配下の client 実装が SSOT。namespace 名は backend 側で `KUBEOPENCODE_NAMESPACE` で受け取る。

### exec Pod 用 namespace

`eval_python` tool が exec Pod を per-execution で起動するため、Pod の lifecycle 管理 (作成、状態取得、削除) とログ取得の権限が必要。具体的な resource と verb は `backend/src/kata_exec/` の client 実装が SSOT。namespace 名は `KATA_EXEC_NAMESPACE` で受け取る。

exec Pod は per-execution で起動し、1 Pod = 1 evaluation で完了後に削除する想定。runtime や NetworkPolicy などの隔離設定はクラスタ側で組み立てる。

## マイグレーション

backend は起動時に `Migrator::up` を自動実行する。`--skip-migration` で抑止、`--migrate-only` でマイグレーションのみ実行して終了するモードがある (例: 事前にジョブで流したい場合)。
