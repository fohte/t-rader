# Deployment

t-rader-backend は home-k8s クラスタ上の `t-rader` namespace に Deployment + Service として配備される。LLM 実行は kubeopencode (別 namespace) に委譲し、backend 自体は Axum プロセスを 1 つだけ起動する。

## Service と接続経路

- backend は port `3000` を listen する (`BACKEND_PORT` で変更可)。
- in-cluster からは `t-rader-backend.t-rader.svc.cluster.local:3000` で到達できる前提。frontend Pod 内 nginx は `/api` を同 Service にリバースプロキシし、SPA から見て同一オリジン構成にする。
- 戦略 Agent (kubeopencode namespace) と personal-bot は in-cluster Service DNS 経由で MCP path (`/mcp/strategy`, `/mcp/mgmt`) を叩く。`MCP_ALLOWED_HOSTS` に Service DNS を追加しないと `rmcp` の DNS rebinding 保護で 403 になる ([`docs/mcp.md`](./mcp.md))。
- 外部公開は frontend のみ。Cloudflare Tunnel → frontend Service → 同 Pod の nginx 経由で backend に到達する。backend Service を直接 Ingress しない。

## 環境変数

必須 (未設定なら起動失敗または production で機能不全) を上、条件付き必須を中、任意を下に置く。

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

## 必須 RBAC

backend ServiceAccount に以下の権限を付与する (詳細な YAML は infra リポジトリで管理)。

### `kubeopencode` namespace

戦略 Agent ライフサイクル (Agent CR / SA / ConfigMap / ExternalSecret の provisioning) と Task CR の投入・監視を backend が直接担う ([`docs/kubeopencode-integration.md`](./kubeopencode-integration.md))。

- `kubeopencode.io/agents`: `create`, `get`, `list`, `watch`, `update`, `patch`, `delete`
- `kubeopencode.io/tasks`: `create`, `get`, `list`, `watch`
- `serviceaccounts`: `create`, `get`, `delete`
- `configmaps`: `create`, `get`, `update`, `patch`, `delete`
- `externalsecrets.external-secrets.io`: `create`, `get`, `delete`

### `t-rader-exec` namespace

`eval_python` tool が exec Pod を per-execution で起動するため、Pod 作成権限が必要。

- `pods`: `create`, `get`, `delete`
- `pods/log`: `get`
- `pods/exec`: `create` (使う場合のみ)

Pod は `runtimeClassName: kata` で microVM 隔離する。NetworkPolicy で egress / ingress を全 deny にすること。

## マイグレーション

backend は起動時に `Migrator::up` を自動実行する。`--skip-migration` で抑止、`--migrate-only` でマイグレーションのみ実行して終了するモードがある (例: 事前にジョブで流したい場合)。
