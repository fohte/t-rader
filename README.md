# t-rader

fohte 個人用の日本株投資プラットフォーム。

戦略 (= 永続ワークスペース) ごとに LLM がアナリスト役となり、ノートとアノテーションを産出する。ユーザーは事後レビュー側に立ち、各アーティファクトに status / コメント / 変更履歴が紐づく。設計の詳細と実装規約は `CLAUDE.md` を参照すること。

## 技術スタック

| レイヤー               | 技術                                                          |
| ---------------------- | ------------------------------------------------------------- |
| Frontend               | React 19, Vite 7, TanStack Router, shadcn/ui, Tailwind CSS v4 |
| Backend                | Rust (Axum)                                                   |
| Agent                  | Node.js/TypeScript (Hono, A2A server)                         |
| DB                     | TimescaleDB (PostgreSQL 17)                                   |
| パッケージマネージャー | pnpm                                                          |
| ツール管理             | mise                                                          |

## 開発環境のセットアップ

### 前提条件

- [mise](https://mise.jdx.dev/) がインストールされていること
- Docker 環境 (Docker Desktop または [Colima](https://github.com/abiosoft/colima)) + docker compose プラグイン

### 起動

```bash
# ツールのインストール
mise install

# 環境変数の設定
cp .env.example .env

# DB を起動 (初回のみ。全 worktree で共有される)。実ポートを反映した .env.runtime も生成される
mise run db-up

# アプリ (backend, frontend, agent) を起動
docker compose up
```

起動後、`docker compose port frontend 5173` で確認したポートでフロントエンドにアクセスできる。

`agent` サービスは `OPENCODE_API_KEY` が未設定だと起動に失敗する。`docker-compose.yml` の `agent` サービスは `.env` を読み込まないため、`.env.local` に設定すること (詳細は下記「環境変数」参照)。

### Git worktree で並列開発する場合

DB は `docker-compose.infra.yml` で 1 つだけ起動し、全 worktree で共有する。
backend / frontend のホストポートは `docker compose up` のたびにランダム割り当てされるため、worktree 間の衝突を気にせずそのまま起動できる。

```bash
# アプリのみ起動 (DB は既に起動済み)
docker compose up

# 割り当てられたポートを確認 (起動中の全コンテナを一覧するなら docker compose ps)
docker compose port backend 3000
docker compose port frontend 5173
docker compose port agent 8080
```

## データベース

- PostgreSQL 17 + TimescaleDB
- マイグレーションは sqlx を使用し、バックエンドの起動時に自動実行される (`sqlx::migrate!()`)
- マイグレーションファイルは `backend/migrations/` に配置

### マイグレーションの追加

`backend/migrations/` に `YYYYMMDDHHMMSS_<name>.sql` 形式のファイルを追加する。次回のバックエンド起動時に自動適用される。

sqlx-cli を使う場合:

```bash
cargo install sqlx-cli --no-default-features --features native-tls,postgres
cd backend
cargo sqlx migrate add <name>
```

### マイグレーションの確認

```bash
# テーブル一覧の確認
docker compose -f docker-compose.infra.yml exec db psql -U t_rader -d t_rader_development -c '\dt'

# hypertable の確認
docker compose -f docker-compose.infra.yml exec db psql -U t_rader -d t_rader_development \
  -c "SELECT hypertable_name FROM timescaledb_information.hypertables;"
```

## API

- `GET /api/health` - ヘルスチェック (DB 接続確認含む)

## Agent サービス

`agent/` は kubeopencode (下記「戦略 Agent reconcile」) を置き換える予定の、A2A (Agent-to-Agent) プロトコルサーバー。A2A server 基盤、internal API、observability に加え、agent-config 取得 (`GET {BACKEND_API_BASE_URL}/api/strategies/{id}/agent-config`) から LangGraph agent 構成、MCP tool 呼び出しまでの戦略実行ロジックを備える。backend からの呼び出しはまだ未接続。

- DB は backend とは別の論理 DB (`t_rader_agent_development` / `t_rader_agent_test`) を同じ Postgres インスタンス上に持つ (`docker-compose.infra.yml` の initdb スクリプトで作成)。initdb は Postgres の data ディレクトリが空の初回起動時にしか実行されないため、既存の共有 `db_data` ボリュームを使っている場合は `docker compose -f docker-compose.infra.yml exec db psql -U t_rader -d t_rader_development -c 'CREATE DATABASE t_rader_agent_development'` 等で手動作成すること (test 用 DB も同様)
- マイグレーションは drizzle-orm を使用し、起動時に自動実行される (`agent/drizzle/`)
- internal API: `POST /internal/tasks` (`{strategy_id, prompt}` -> `{task_id}`) / `GET /internal/tasks/{task_id}` (-> `{task_id, state, result_text?, error_kind?}`)

```bash
# agent 単体でテスト実行 (DB 統合テストは TEST_DATABASE_URL 未設定時は自動 skip)
cd agent && pnpm test

# DB 統合テストを含めて実行する場合。ポートは .env.runtime の DATABASE_URL か
# `docker compose -f docker-compose.infra.yml port db 5432` で確認する
cd agent && TEST_DATABASE_URL=postgres://t_rader:t_rader@localhost:<port>/t_rader_agent_test pnpm test
```

## プロジェクト構成

```
├── frontend/          # React SPA
│   ├── src/
│   │   ├── components/  # UI コンポーネント
│   │   ├── routes/      # TanStack Router のファイルベースルーティング
│   │   └── main.tsx     # エントリーポイント
│   └── package.json
├── backend/           # Rust Axum サーバー
│   └── migrations/    # sqlx マイグレーション (起動時に自動実行)
├── agent/             # Node/TS 戦略 Agent サービス (A2A server)
│   ├── src/
│   └── drizzle/       # drizzle-orm マイグレーション (起動時に自動実行)
├── docker-compose.yml        # アプリ (backend, frontend, agent) 定義
├── docker-compose.infra.yml  # インフラ (DB) 定義。全 worktree で共有
└── .mise.toml                # ツールバージョン管理
```

## npm スクリプト (frontend/, agent/)

```bash
pnpm run dev        # 開発サーバー
pnpm run build      # プロダクションビルド
pnpm run test       # 型チェック + ユニットテスト
pnpm run lint       # ESLint
pnpm run format     # ESLint + Prettier によるフォーマット
```

## 環境変数

| 変数                     | 説明                                                                                                                                                                                            | デフォルト              |
| ------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------- |
| `DATABASE_URL`           | PostgreSQL 接続 URL (`mise run db-up` が実ポートを反映して `.env.runtime` に自動生成する)                                                                                                       | -                       |
| `POSTGRES_USER`          | DB ユーザー名                                                                                                                                                                                   | `t_rader`               |
| `POSTGRES_PASSWORD`      | DB パスワード                                                                                                                                                                                   | `t_rader`               |
| `POSTGRES_DB`            | DB 名                                                                                                                                                                                           | `t_rader_development`   |
| `BACKEND_PORT`           | backend プロセスのリッスンポート (`cargo run` 直接実行時や本番で使用。docker compose 経由のホスト側ポートはランダム割り当てのため無関係)                                                        | `3000`                  |
| `TRADER_AGENT_PORT`      | agent プロセスのリッスンポート (`pnpm dev` 直接実行時や本番で使用。docker compose 経由のホスト側ポートはランダム割り当てのため無関係)                                                           | `8080`                  |
| `TRADER_AGENT_URL`       | agent が自身の A2A Agent Card に載せる URL                                                                                                                                                      | -                       |
| `TRADER_AGENT_API_URL`   | backend が戦略タスクを投入する t-rader-agent の internal API base URL。値 `disabled` は dev 用 sentinel で agent へのタスク投入を無効化する                                                     | -                       |
| `TRADER_AGENT_API_TOKEN` | backend が agent の internal API 呼び出し時に送る bearer token (agent 側の `INTERNAL_API_TOKEN` と同じ値、`TRADER_AGENT_API_URL=disabled` の場合は不要)                                         | -                       |
| `INTERNAL_API_TOKEN`     | backend -> agent の internal API 呼び出しを認証する bearer token                                                                                                                                | -                       |
| `BACKEND_WEBHOOK_URL`    | agent -> backend の push notification 送信先 URL                                                                                                                                                | -                       |
| `BACKEND_WEBHOOK_TOKEN`  | agent -> backend の push notification 送信を認証する bearer token                                                                                                                               | -                       |
| `AGENT_WEBHOOK_TOKEN`    | backend が agent からの push notification を認証する bearer token (`BACKEND_WEBHOOK_TOKEN` と同じ値)                                                                                            | -                       |
| `BACKEND_API_BASE_URL`   | agent が戦略の AGENTS.md / skills / model を取得する backend のベース URL                                                                                                                       | -                       |
| `STRATEGY_MCP_URL`       | agent が strategy tool 群に接続する backend の MCP エンドポイント (下記「戦略 Agent reconcile」の同名変数は backend 側の別用途)                                                                 | -                       |
| `OPENCODE_API_KEY`       | agent が戦略 Agent の LLM (OpenCode Go) 呼び出しに使う API キー。未設定だと agent の起動に失敗する。docker-compose の `agent` サービスは `.env` を読み込まないため、`.env.local` に設定すること | -                       |
| `JQUANTS_API_KEY`        | J-Quants API キー (`DATA_PROVIDER=jquants` 時に使用)                                                                                                                                            | -                       |
| `VITE_API_URL`           | Vite 開発サーバーのプロキシ先 URL                                                                                                                                                               | `http://localhost:3000` |
| `API_BACKEND_URL`        | nginx リバースプロキシの転送先 URL (本番用、実行時に設定必須)                                                                                                                                   | -                       |
| `NGINX_RESOLVER`         | nginx の DNS リゾルバ (Kubernetes: kube-dns アドレス、実行時に設定必須)                                                                                                                         | -                       |
| `MCP_ALLOWED_HOSTS`      | MCP server が受理する `Host` header の追加許可リスト (カンマ区切り)                                                                                                                             | -                       |

### DataProvider 切替

`DATA_PROVIDER` 環境変数で価格データの取得元を選ぶ。デフォルト (未設定) は `jquants`。

| 値        | 必要な追加変数                                                                                 | 用途                                                            |
| --------- | ---------------------------------------------------------------------------------------------- | --------------------------------------------------------------- |
| `jquants` | `JQUANTS_API_KEY` (未設定時は DataProvider なしで起動)                                         | J-Quants API (無料枠は 12 週遅延あり)                           |
| `ibkr`    | `IBKR_BASE_URL` (任意), `IBKR_SESSION_TOKEN` (任意), `IBKR_EXCHANGE` (任意、デフォルト `TSEJ`) | IBKR Client Portal Web API。Gateway を別途常駐させて URL を指す |
| `none`    | (なし)                                                                                         | DataProvider を無効化。データ取得系エンドポイントは 503 を返す  |

IBKR を使う場合は Client Portal Gateway を VKE クラスタ等に常駐させ、その HTTP エンドポイントを `IBKR_BASE_URL` に設定する (例: `https://ibkr-gateway:5000/v1/api`)。秘密鍵相当の API キーは存在せず、認証は Gateway 側の Web ログインで維持される。

### 戦略 Agent reconcile

kubeopencode operator (Agent CR / Task CR) ベースの戦略実行機構。将来的に上記の `agent/` (LangGraph JS ベース) へ移行予定だが、移行が完了するまではこちらが本番の実行経路。

`KUBEOPENCODE_API_URL` が `disabled` でない (= 実 kube cluster に接続する) 場合、戦略 Agent の reconcile に以下が必要になる。

| 変数                                    | 必須/任意 | 用途                                                                                                                       |
| --------------------------------------- | --------- | -------------------------------------------------------------------------------------------------------------------------- |
| `STRATEGY_MCP_URL`                      | 必須      | Agent CR の `spec.config.mcp.t-rader-strategy.url`。例: `http://t-rader-backend.t-rader/mcp/strategy`                      |
| `STRATEGY_AGENT_SSM_PARAMETER_TEMPLATE` | 任意      | SSM パラメータ key の template。`{name}` が Agent 名で置換される。デフォルト `/infra/kubeopencode/{name}-opencode-api-key` |
| `STRATEGY_AGENT_MODEL`                  | 任意      | 戦略 Agent の primary model。デフォルト `opencode-go/minimax-m3`                                                           |
| `STRATEGY_AGENT_SMALL_MODEL`            | 任意      | 戦略 Agent の small model。デフォルト `opencode-go/deepseek-v4-flash`                                                      |

dev では `KUBEOPENCODE_API_URL=disabled` を使えば上記は不要。

## Deployment と外部連携

- [`docs/mcp.md`](./docs/mcp.md): `/mcp/mgmt` と `/mcp/strategy` の tool 一覧、session 永続化、`MCP_ALLOWED_HOSTS` の挙動
- [`docs/deployment.md`](./docs/deployment.md): 必須 env、backend が要求する権限、Service port
- [`docs/kubeopencode-integration.md`](./docs/kubeopencode-integration.md): kubeopencode operator との連携で backend が依存する seam
