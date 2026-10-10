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

Graphile Worker 管理 UI は reverse proxy header 認証を設定できます。header 名と照合値の両方を設定するとその header で認証し、両方を空欄にすると認証なしで backend は `127.0.0.1` に bind します。片方だけ設定した場合は起動に失敗します。この loopback 設定で UI を使う場合は backend をホスト上で直接起動してください。Docker Compose の port publish からは UI に到達できません。Docker Compose で使う場合は認証 header の組を設定し、信頼済み proxy を経由してください。

```bash
# ツールのインストール
mise install

# 環境変数の設定
cp .env.example .env

# DB と Redis を起動 (全 worktree で共有される)
mise run db:up

# アプリ (backend, frontend, agent) を起動
docker compose -f docker-compose.app.yml up
```

起動後、`docker compose -f docker-compose.app.yml port frontend 5173` で確認したポートでフロントエンドにアクセスできる。

`agent` サービスは `LLM_API_KEY` が未設定だと起動に失敗する。`docker-compose.app.yml` の `agent` サービスは `.env` を読み込まないため、`.env.local` に設定すること (詳細は下記「環境変数」参照)。

### Git worktree で並列開発する場合

DB と Redis は `compose.yaml` と `compose.override.yaml` で 1 つだけ起動し、全 worktree で共有する。
アプリのホストポートは `docker compose -f docker-compose.app.yml up` のたびにランダム割り当てされるため、worktree 間の衝突を気にせずそのまま起動できる。

```bash
# アプリのみ起動 (DB は既に起動済み)
docker compose -f docker-compose.app.yml up

# 割り当てられたポートを確認 (アプリコンテナの一覧: docker compose -f docker-compose.app.yml ps)
docker compose -f docker-compose.app.yml port backend 3000
docker compose -f docker-compose.app.yml port backend 3001
docker compose -f docker-compose.app.yml port frontend 5173
docker compose -f docker-compose.app.yml port agent 8080
```

## データベース

- PostgreSQL 17 + TimescaleDB と Redis は `mise run db:up` で起動する
- backend と agent にそれぞれ dev / test DB が作成され、各 package の `.mise.toml` が `DATABASE_URL` と `TEST_DATABASE_URL` を解決する
- DB schema docs は使い捨て DB に migration を適用して生成する
- backend の migration は SeaORM を使用する

```bash
mise run db:doc
```

### 既存の共有 DB を使っている場合

既存の DB volume はそのまま再利用します。現行構成の role と開発 database 名を、新構成の名前に一度だけ変更してください。アプリを停止し、DB コンテナを起動した状態で次を実行します。

```bash
docker compose exec -T db psql -U "t-rader" -d postgres <<'SQL'
ALTER ROLE "t-rader" RENAME TO t_rader;
ALTER ROLE t_rader PASSWORD 't_rader';
ALTER DATABASE "t-rader_backend_dev" RENAME TO t_rader_backend_dev;
ALTER DATABASE "t-rader_agent_dev" RENAME TO t_rader_agent_dev;
SQL
```

切り替え後に `mise run db:up` を実行すると test DB が作成されます。volume の削除は不要です。

### マイグレーションの追加

backend の migration は CLI から生成する。

```bash
cd backend/migration && cargo run -- generate <name>
```

agent の Drizzle migration は次のコマンドで生成し、適用する。

```bash
cd agent && pnpm run db:generate
cd agent && pnpm run db:migrate
```

### マイグレーションの確認

```bash
# テーブル一覧の確認
docker compose exec db psql -U t_rader -d t_rader_backend_dev -c '\dt'

# hypertable の確認
docker compose exec db psql -U t_rader -d t_rader_backend_dev \
  -c "SELECT hypertable_name FROM timescaledb_information.hypertables;"
```

## API

- `GET /api/health` - ヘルスチェック (DB 接続確認含む)

## Agent サービス

`agent/` は A2A (Agent-to-Agent) プロトコルサーバー。A2A server 基盤、internal API、observability に加え、agent-config 取得 (`GET {BACKEND_API_BASE_URL}/api/agent-configs/{purpose}/agent-config`) から LangGraph agent 構成、MCP tool 呼び出しまでの戦略実行ロジックを備える。

- DB は backend とは別の論理 DB (`t_rader_agent_dev` / `t_rader_agent_test`) を同じ Postgres インスタンス上に持つ。`mise run db:up` が作成する
- マイグレーションは drizzle-orm を使用し、起動時に自動実行される (`agent/drizzle/`)
- internal API: `POST /internal/tasks` (`{strategy_id, prompt}` -> `{task_id}`) / `GET /internal/tasks/{task_id}` (-> `{task_id, state, result_text?, error_message?, error_kind?}`)

```bash
# agent 単体でテスト実行 (`agent/.mise.toml` が TEST_DATABASE_URL を設定する)
cd agent && pnpm test
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
│   └── migration/     # SeaORM マイグレーション
├── agent/             # Node/TS 戦略 Agent サービス (A2A server)
│   ├── src/
│   └── drizzle/       # drizzle-orm マイグレーション (起動時に自動実行)
├── compose.yaml              # DB 定義。全 worktree で共有
├── compose.override.yaml     # TimescaleDB と Redis の設定
├── docker-compose.app.yml    # アプリ (backend, frontend, agent) 定義
└── .mise.toml                # ツールバージョン管理
```

## npm スクリプト

```bash
# frontend/, agent/ 個別
cd frontend && pnpm run dev    # 開発サーバー
cd frontend && pnpm run build  # プロダクションビルド

# ルートから実行 (workspace 一括)
pnpm run test    # 全 package の型チェック + ユニットテスト
pnpm run lint     # ESLint
pnpm run format   # ESLint + Prettier によるフォーマット
```

## 環境変数

| 変数                                                                                       | 説明                                                                                                                                                                                                                                                 | デフォルト                   |
| ------------------------------------------------------------------------------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------- |
| `DATABASE_URL`                                                                             | package ごとの PostgreSQL 接続 URL (`backend/.mise.toml` / `agent/.mise.toml` が解決する)                                                                                                                                                            | -                            |
| `TEST_DATABASE_URL`                                                                        | package ごとの test DB 接続 URL (`backend/.mise.toml` / `agent/.mise.toml` が解決する)                                                                                                                                                               | -                            |
| `REDIS_URL`                                                                                | Redis 接続 URL (`.mise.toml` が `scripts/redis-url` を都度実行して解決する)                                                                                                                                                                          | -                            |
| `BACKEND_PORT`                                                                             | backend プロセスのリッスンポート (`cargo run -p backend` 直接実行時や本番で使用。docker compose 経由のホスト側ポートはランダム割り当てのため無関係)                                                                                                  | `3000`                       |
| `TRADER_AGENT_PORT`                                                                        | agent プロセスのリッスンポート (`pnpm dev` 直接実行時や本番で使用。docker compose 経由のホスト側ポートはランダム割り当てのため無関係)                                                                                                                | `8080`                       |
| `TRADER_AGENT_URL`                                                                         | agent が自身の A2A Agent Card に載せる URL                                                                                                                                                                                                           | -                            |
| `TRADER_AGENT_API_URL`                                                                     | backend が戦略タスクを投入する t-rader-agent の internal API base URL。値 `disabled` は dev 用 sentinel で agent へのタスク投入を無効化する                                                                                                          | -                            |
| `TRADER_AGENT_API_TOKEN`                                                                   | backend が agent の internal API 呼び出し時に送る bearer token (agent 側の `INTERNAL_API_TOKEN` と同じ値、`TRADER_AGENT_API_URL=disabled` の場合は不要)                                                                                              | -                            |
| `INTERNAL_API_TOKEN`                                                                       | backend -> agent の internal API 呼び出しを認証する bearer token                                                                                                                                                                                     | -                            |
| `BACKEND_WEBHOOK_TOKEN`                                                                    | agent -> backend の push notification 送信を認証する bearer token                                                                                                                                                                                    | -                            |
| `AGENT_WEBHOOK_TOKEN`                                                                      | backend が agent からの push notification を認証する bearer token (`BACKEND_WEBHOOK_TOKEN` と同じ値)                                                                                                                                                 | -                            |
| `BACKEND_API_BASE_URL`                                                                     | agent が backend API、webhook、MCP endpoint に接続するベース URL                                                                                                                                                                                     | -                            |
| `LLM_API_KEY`                                                                              | agent が戦略 Agent の LLM 呼び出しに使う API キー (`LLM_BASE_URL` を差し替えた場合はその接続先の API キー)。未設定だと agent の起動に失敗する。`docker-compose.app.yml` の `agent` サービスは `.env` を読み込まないため、`.env.local` に設定すること | -                            |
| `LLM_BASE_URL`                                                                             | agent が戦略 Agent の LLM 呼び出しに使う OpenAI 互換エンドポイントの base URL。任意の互換エンドポイント (LiteLLM Proxy 等) に差し替えられる                                                                                                          | OpenCode Go のエンドポイント |
| `JQUANTS_API_KEY`                                                                          | J-Quants API キー (`DATA_PROVIDER=jquants` 時に使用)。設定する場合は provider の選択にかかわらず `JQUANTS_PLAN` が必要                                                                                                                               | -                            |
| `JQUANTS_PLAN`                                                                             | J-Quants の契約プラン (`free` / `light` / `standard` / `premium`)。API キー設定時に未設定または不正な値だと backend の起動に失敗する。API キー未設定時は無視する                                                                                     | -                            |
| `FIRECRAWL_API_KEY`                                                                        | ニュース記事本文を取得する Firecrawl API キー。未設定なら本文取得 job を登録しない                                                                                                                                                                   | -                            |
| `TAVILY_API_KEY`                                                                           | MCP の `search_web` が Tavily で Web 検索するための API キー。未設定なら `search_web` はエラーを返す                                                                                                                                                 | -                            |
| `VITE_API_URL`                                                                             | Vite 開発サーバーのプロキシ先 URL                                                                                                                                                                                                                    | `http://localhost:3000`      |
| `API_BACKEND_URL`                                                                          | nginx リバースプロキシの転送先 URL (本番用、実行時に設定必須)                                                                                                                                                                                        | -                            |
| `NGINX_RESOLVER`                                                                           | nginx の DNS リゾルバ (Kubernetes: kube-dns アドレス、実行時に設定必須)                                                                                                                                                                              | -                            |
| `MCP_ALLOWED_HOSTS`                                                                        | MCP server が受理する `Host` header の追加許可リスト (カンマ区切り)                                                                                                                                                                                  | -                            |
| `GRAPHILE_WORKER_ADMIN_UI_AUTH_HEADER_NAME` / `GRAPHILE_WORKER_ADMIN_UI_AUTH_HEADER_VALUE` | backend の `worker` / `both` で管理 UI が照合する reverse proxy header 名と値。両方を空欄にすると loopback で認証なし、片方だけの設定は起動失敗                                                                                                      | -                            |
| `GRAPHILE_WORKER_ADMIN_UI_PORT`                                                            | Graphile Worker 管理 UI の listen port                                                                                                                                                                                                               | `3001`                       |

### DataProvider 切替

`DATA_PROVIDER` 環境変数で価格データの取得元を選ぶ。デフォルト (未設定) は `jquants`。

| 値        | 必要な追加変数                                                                                 | 用途                                                                                                                              |
| --------- | ---------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------- |
| `jquants` | `JQUANTS_API_KEY` (未設定時は DataProvider なしで起動)、設定時は `JQUANTS_PLAN` も必須         | J-Quants API。契約プランに応じて取得可能期間とレート制限を決める。プランの検証は API キー設定時に provider の選択にかかわらず行う |
| `ibkr`    | `IBKR_BASE_URL` (任意), `IBKR_SESSION_TOKEN` (任意), `IBKR_EXCHANGE` (任意、デフォルト `TSEJ`) | IBKR Client Portal Web API。Gateway を別途常駐させて URL を指す                                                                   |
| `none`    | (なし)                                                                                         | DataProvider を無効化。データ取得系エンドポイントは 503 を返す                                                                    |

IBKR を使う場合は Client Portal Gateway を VKE クラスタ等に常駐させ、その HTTP エンドポイントを `IBKR_BASE_URL` に設定する (例: `https://ibkr-gateway:5000/v1/api`)。秘密鍵相当の API キーは存在せず、認証は Gateway 側の Web ログインで維持される。

## Deployment と外部連携

- [`docs/backend-architecture.md`](./docs/backend-architecture.md): backend の crate 構成、依存規則、境界の責務
- [`docs/mcp.md`](./docs/mcp.md): `/mcp/mgmt` と `/mcp/strategy` の tool 一覧、session 管理方針、`MCP_ALLOWED_HOSTS` の挙動
- [`docs/deployment.md`](./docs/deployment.md): 必須 env、backend が要求する権限、Service port
