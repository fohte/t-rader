# Deployment

t-rader-backend は Kubernetes クラスタ上に Deployment + Service として配備する。backend 自体は Axum プロセスを 1 つだけ起動し、LLM 実行は kubeopencode operator (別 namespace) に委譲する。本ドキュメントは backend を任意の Kubernetes クラスタで動かすために必要な要件を記述する。

## Service と接続経路

- backend は port `3000` を listen する (`BACKEND_PORT` で変更可)。
- frontend Pod 内 nginx が `/api` を backend Service にリバースプロキシし、SPA から見て同一オリジン構成にする (frontend の Dockerfile と `nginx.conf.template` 参照)。
- MCP クライアント (戦略 Agent / 管理 MCP の外部コントロールプレーンクライアント) は in-cluster Service DNS で MCP path (`/mcp/strategy`, `/mcp/mgmt`) を叩く。`MCP_ALLOWED_HOSTS` に backend Service の DNS 名を追加しないと `rmcp` の DNS rebinding 保護で 403 になる ([`docs/mcp.md`](./mcp.md))。
- backend Service を直接 Ingress しない。外部公開は frontend のみで、Ingress や Tunnel など外部経路は環境に応じて構成する。

## 環境変数

| 区分              | 変数                     | 意味                                                                                                                                             | 未設定時の挙動                                                                                              |
| ----------------- | ------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------ | ----------------------------------------------------------------------------------------------------------- |
| 必須              | `DATABASE_URL`           | PostgreSQL 接続 URL。起動時にマイグレーションを自動実行する                                                                                      | 起動失敗                                                                                                    |
| 必須              | `AGENT_WEBHOOK_TOKEN`    | agent からの webhook 通知 (`POST /api/agent-tasks/notifications`) を認証する bearer token。agent 側の `BACKEND_WEBHOOK_TOKEN` と同じ値にすること | 起動失敗                                                                                                    |
| 必須 (production) | `KUBEOPENCODE_API_URL`   | kube-apiserver の base URL (in-cluster なら `https://kubernetes.default.svc`)                                                                    | warning ログを出して `DisabledKubeopencodeClient` で起動。Task CR 投入は全て失敗                            |
| 必須 (production) | `TRADER_AGENT_API_URL`   | t-rader-agent internal API の base URL (例: `http://t-rader-agent.t-rader/internal`)                                                             | 起動失敗。`disabled` (dev 用 sentinel) を明示すると warning ログを出して agent task client を無効化して起動 |
| 条件付き          | `TRADER_AGENT_API_TOKEN` | `TRADER_AGENT_API_URL` が実 URL の場合に必要な bearer token (agent 側の `INTERNAL_API_TOKEN` と同じ値)                                           | 起動失敗                                                                                                    |
| 必須 (in-cluster) | `MCP_ALLOWED_HOSTS`      | MCP server が受理する `Host` ヘッダの追加許可リスト (カンマ区切り)                                                                               | in-cluster Service DNS が 403 で弾かれる ([`docs/mcp.md`](./mcp.md))                                        |
| 必須 (production) | `KATA_EXEC_API_URL`      | Kata Containers exec Pod を起動するための kube-apiserver base URL                                                                                | warning ログを出して `eval_python` tool が disabled になる                                                  |
| 任意              | `DATA_PROVIDER`          | 価格データ取得元の選択 (`jquants` / `ibkr` / `none`)。未設定なら `jquants` 扱い                                                                  | `jquants` で起動                                                                                            |
| 条件付き          | `JQUANTS_API_KEY`        | `DATA_PROVIDER=jquants` 時のみ必要                                                                                                               | DataProvider なしで起動 (`query_data` が動かない)                                                           |
| 条件付き          | `IBKR_BASE_URL` 等       | `DATA_PROVIDER=ibkr` 時に Gateway 接続情報を設定                                                                                                 | デフォルト値で初期化を試行                                                                                  |
| 任意              | `BACKEND_PORT`           | listen port                                                                                                                                      | `3000` で listen                                                                                            |

kubeopencode / kata-exec の追加の任意変数 (`KUBEOPENCODE_NAMESPACE`, `KATA_EXEC_NAMESPACE`, `KATA_EXEC_IMAGE`, `KATA_EXEC_TOKEN`, `KATA_EXEC_DEFAULT_TIMEOUT_SECS`, `KATA_EXEC_MAX_OUTPUT_BYTES` 等) は backend のソース (`backend/src/kubeopencode/`, `backend/src/kata_exec/`) を参照。in-cluster で動かす場合 token / CA は ServiceAccount のものを自動で読む。

## backend が要求する権限

backend ServiceAccount に以下の Role を付与する。namespace 名は backend 側で env (`KUBEOPENCODE_NAMESPACE` / `KATA_EXEC_NAMESPACE`) で受け取るため任意。

### 戦略 Agent ランタイム namespace

`submit_strategy_task` から Task CR を作成し、watcher で status を取得するために以下が必要:

```yaml
apiVersion: rbac.authorization.k8s.io/v1
kind: Role
metadata:
  name: t-rader-kubeopencode
  namespace: <KUBEOPENCODE_NAMESPACE>
rules:
  - apiGroups: ['kubeopencode.io']
    resources: ['tasks']
    verbs: ['create', 'get']
```

戦略 Agent CR と関連 ServiceAccount / ConfigMap / ExternalSecret は現状 backend からは reconcile しないため、それらの権限は不要 ([`docs/kubeopencode-integration.md`](./kubeopencode-integration.md))。

### exec Pod 用 namespace

`eval_python` tool が exec Pod を per-execution で起動・回収するため以下が必要:

```yaml
apiVersion: rbac.authorization.k8s.io/v1
kind: Role
metadata:
  name: t-rader-kata-exec
  namespace: <KATA_EXEC_NAMESPACE>
rules:
  - apiGroups: ['']
    resources: ['pods']
    verbs: ['create', 'get', 'list', 'watch', 'delete']
  - apiGroups: ['']
    resources: ['pods/log']
    verbs: ['get']
```

exec Pod は untrusted な LLM 生成コードを実行するため、以下を namespace 側で構成すること:

- `RuntimeClass` `kata` (microVM 等価の隔離 runtime) を当てる前提で backend が Pod spec を組み立てる。クラスタにこの RuntimeClass を登録すること
- NetworkPolicy で `<KATA_EXEC_NAMESPACE>` の egress と ingress を全 deny にする
- Pod Security Admission を `restricted` 相当に設定する

Pod spec 側の不変条件 (`automountServiceAccountToken: false`、`runAsNonRoot`、`readOnlyRootFilesystem`、`capabilities.drop: [ALL]`、`activeDeadlineSeconds`、CPU / memory / ephemeral-storage の limits) は backend が build_pod_manifest で固定する。

## マイグレーション

backend は起動時に `Migrator::up` を自動実行する。`--skip-migration` で抑止、`--migrate-only` でマイグレーションのみ実行して終了するモードがある (例: 事前にジョブで流したい場合)。
