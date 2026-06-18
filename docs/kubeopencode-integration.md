# kubeopencode integration

t-rader-backend と kubeopencode operator の連携仕様。

戦略 Agent ランタイムは home-k8s 上の `kubeopencode` namespace で動く。1 戦略 = 1 Agent CR で `opencode serve` を常駐させ、戦略タスクは Task CR 単位で attach 実行する。

## 責務境界

| 項目                                               | 担当                  |
| -------------------------------------------------- | --------------------- |
| Agent CR (`kubeopencode.io/v1alpha1`) の reconcile | **backend**           |
| Agent 用 ServiceAccount の作成・削除               | **backend**           |
| 戦略ごとの ConfigMap (`AGENTS.md` / skills)        | **backend**           |
| 戦略 Agent 用 ExternalSecret                       | **backend**           |
| Task CR の作成 (`submit_strategy_task` 経由)       | **backend**           |
| Task CR の status 監視 (`strategy_task` への反映)  | **backend**           |
| Agent Pod の起動・idle suspend                     | kubeopencode operator |
| Task Pod の起動 (Agent への attach)                | kubeopencode operator |
| Agent CR スキーマ自体の所有                        | kubeopencode operator |

infra リポジトリの責務は kubeopencode operator 本体のデプロイと、backend ServiceAccount への RBAC 付与のみ。戦略ごとのテンプレートや `values.yaml` 上の戦略エントリは持たない (backend が単独で reconcile する)。

## Agent CR

### 命名規約

戦略 Agent CR の名前は backend が単独で決定する。infra 側で名前を再構築する必要はない。

```
metadata.name = strategy-{uuid_no_dashes}
```

例: `strategy_id = 12345678-1234-5678-1234-567812345678` → `strategy-12345678123456781234567812345678`

`uuid_no_dashes` 形式は RFC 1123 label の制約 (lowercase alphanumeric + `-`, ≤ 63 文字) を自然に満たす。

### spec の構造

backend が apply する Agent CR は以下の field を持つ:

- `spec.persistence.sessions`: SQLite session の永続化先 (PVC)
- `spec.standby.idleTimeout`: idle suspend のしきい値
- `spec.credentials`: `OPENCODE_API_KEY` を ExternalSecret 経由で注入する設定
- `spec.config`: 戦略実行 MCP の URL (`http://t-rader-backend.t-rader.svc.cluster.local:3000/mcp/strategy`) と `x-strategy-id` ヘッダを埋めた `opencode.json`

Agent Pod の cwd には backend が作成した ConfigMap (戦略の `AGENTS.md` と skills 群) を mount する。本文 / データ / 履歴は LLM が戦略実行 MCP の tool 経由で都度取りに行く。

### reconcile タイミング

- `POST /api/strategies` で戦略 row を `agent_status = Pending` で挿入した直後に非同期で Kubernetes API に apply。成功時 `agent_status = Ready`、失敗時 `agent_status = Failed` + `agent_error`。
- `DELETE /api/strategies/:id` で Agent CR を削除する。SA / ConfigMap / ExternalSecret は ownerReference で Agent CR に紐付いており、operator (kubeopencode + 標準 GC) が下流リソースを GC する。
- `submit_strategy_task` は `agent_status` を pre-check し、`Ready` 以外なら明示的なエラー (`strategy agent not ready: <reason>`) を返す。

reconcile の DB 表現 (カラム: `agents_md text`, `skills jsonb`, `agent_status text`, `agent_error text`) は `strategy` テーブルに同居する。

### Agent reconcile の入力ソース

`AGENTS.md` (戦略方針 / 制約 / KPI) と skills (`{name: markdown_body}` の jsonb) は `strategy` テーブルから取る。public リポジトリには載せない (戦略ごとの個別内容を含むため)。空のままなら placeholder を配布する。

## Task CR

### 投入経路

`submit_strategy_task` (管理 MCP) が `kubeopencode` namespace に Task CR を 1 件作成する。

```yaml
apiVersion: kubeopencode.io/v1alpha1
kind: Task
metadata:
  name: <kubeopencode_task_name>
  namespace: kubeopencode
spec:
  agentRef:
    name: <agent_name> # strategy-{uuid_no_dashes}
  description: <prompt>
```

### backend が依存する field

- `metadata.name`: backend 側で `strategy_task.kubeopencode_task_name` に保存し、以後の status 取得キーとして使う。形式は `t-rader-<strategy_short>-<random_short>` (戦略 id 先頭 8 文字 + random 8 文字)。
- `spec.agentRef.name`: 上記の Agent CR 命名規約に従う。
- `status.phase`: `Pending` / `Running` / `Completed` (`Succeeded`) / `Failed` を期待。backend は小文字化して `TaskPhase` enum にマップする。未知の値は無視する。
- `status.message`: 失敗時のエラー要約。`get_strategy_task_status` の `error_summary` に流す。

backend は watcher (`backend/src/mcp/watcher.rs`) で `Pending` / `Running` の Task を一定間隔で poll し、`strategy_task` テーブルの `phase` カラムに反映する。

### 削除ポリシー

Task CR は kubeopencode operator が完了後に保持する。backend 側からは削除しない。
