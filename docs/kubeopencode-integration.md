# kubeopencode integration

t-rader-backend が依存する [kubeopencode](https://github.com/fohte/kubeopencode) operator との連携仕様。CR の正式仕様は kubeopencode 側のドキュメントが SSOT で、ここでは backend が依存する seam (どの CR をどう操作するか、どの field を読むか) を記述する。

戦略 Agent ランタイムは別の namespace (backend は `KUBEOPENCODE_NAMESPACE` で受け取る) に置く。1 戦略 = 1 Agent CR で `opencode serve` を常駐させ、戦略タスクは Task CR 単位で attach 実行する。

## 責務境界

| 項目                                               | 担当                  |
| -------------------------------------------------- | --------------------- |
| Agent CR (`kubeopencode.io/v1alpha1`) の reconcile | **backend**           |
| Agent 用 ServiceAccount の作成と削除               | **backend**           |
| 戦略ごとの ConfigMap (`AGENTS.md` と skills)       | **backend**           |
| 戦略 Agent 用 ExternalSecret                       | **backend**           |
| Task CR の作成 (`submit_strategy_task` 経由)       | **backend**           |
| Task CR の status 監視 (`strategy_task` への反映)  | **backend**           |
| Agent Pod の起動と idle suspend                    | kubeopencode operator |
| Task Pod の起動 (Agent への attach)                | kubeopencode operator |
| Agent CR スキーマ自体の所有                        | kubeopencode operator |

戦略ごとのテンプレートや戦略エントリは backend が単独で reconcile する設計。

## Agent CR

### 命名規約

戦略 Agent CR の名前は backend が単独で決定する。生成ロジックは `backend/src/mcp/mgmt.rs` 内の Agent CR 命名ヘルパが SSOT であり、外側からは「戦略 id から決まる一意な名前」として opaque に扱う。

### spec の構造

backend が apply する Agent CR の spec は kubeopencode 側スキーマの subset を埋める形になる。backend が組み立てる主要 field は次のとおり:

- session の永続化先 (PVC) と idle suspend 設定
- `OPENCODE_API_KEY` の ExternalSecret 注入
- 戦略実行 MCP の URL (in-cluster の backend Service DNS) と `x-strategy-id` ヘッダを埋めた `opencode.json`

具体的な field 名と階層は kubeopencode 側スキーマに従う (drift 防止のため backend のコードを SSOT とする)。Agent Pod の cwd には backend が作成した ConfigMap (戦略の `AGENTS.md` と skills 群) を mount する。本文 / データ / 履歴は LLM が戦略実行 MCP の tool 経由で都度取りに行く。

### reconcile タイミング

backend は Agent CR と関連リソース (SA、ConfigMap、ExternalSecret) の自動 reconcile を行わない。`submit_strategy_task` も Agent の ready 状態を pre-check せず Task CR の作成のみを行う。

## Task CR

### 投入経路

`submit_strategy_task` (管理 MCP) が戦略 Agent ランタイム namespace に Task CR を 1 件作成し、対応する Agent CR に紐付ける。Task の名前は backend 側で生成して `strategy_task.kubeopencode_task_name` に保存し、以後の status 取得キーとして使う。生成ロジックは `backend/src/mcp/mgmt.rs` が SSOT。

### backend が依存する field

backend は spec 側で「Task の名前」「対象戦略の Agent CR への紐付け」「prompt」を書き込み、status 側で「実行 phase」と「失敗時のエラー要約」を読む。具体的な field 名と phase 値の小文字化マッピングは `backend/src/kubeopencode/client.rs` の `TaskPhase` 周辺が SSOT。未知の phase 値は無視する。

backend は watcher (`backend/src/mcp/watcher.rs`) で未完の Task を一定間隔で poll し、`strategy_task` テーブルの `phase` カラムに反映する。

### 削除ポリシー

Task CR は kubeopencode operator が完了後に保持する。backend 側からは削除しない。
