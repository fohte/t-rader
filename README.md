# t-rader

fohte 個人用の日本株投資プラットフォーム。

戦略 (= 永続ワークスペース) ごとに LLM がアナリスト役となり、ノートとアノテーションを産出する。ユーザーは事後レビュー側に立ち、各アーティファクトに status / コメント / 変更履歴が紐づく。設計の詳細と実装規約は `CLAUDE.md` を参照すること。

## 技術スタック

| レイヤー               | 技術                                                          |
| ---------------------- | ------------------------------------------------------------- |
| Frontend               | React 19, Vite 7, TanStack Router, shadcn/ui, Tailwind CSS v4 |
| Backend                | Rust (Axum)                                                   |
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

# DB を起動 (初回のみ。全 worktree で共有される)
docker compose -f docker-compose.infra.yml up -d

# アプリ (backend, frontend) を起動
docker compose up
```

起動後、http://localhost:5173 でフロントエンドにアクセスできる。

### Git worktree で並列開発する場合

DB は `docker-compose.infra.yml` で 1 つだけ起動し、全 worktree で共有する。各 worktree では `.env` でポートを変えてアプリのみ起動する。

worktree 側の `.env` でポートを変更:

```dotenv
BACKEND_PORT=3001
FRONTEND_PORT=5174
```

```bash
# アプリのみ起動 (DB は既に起動済み)
docker compose up
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
├── docker-compose.yml        # アプリ (backend, frontend) 定義
├── docker-compose.infra.yml  # インフラ (DB) 定義。全 worktree で共有
└── .mise.toml                # ツールバージョン管理
```

## npm スクリプト (frontend/)

```bash
pnpm run dev        # Vite 開発サーバー
pnpm run build      # プロダクションビルド
pnpm run test       # 型チェック + ユニットテスト
pnpm run lint       # ESLint
pnpm run format     # ESLint + Prettier によるフォーマット
```

## 環境変数

| 変数                | 説明                                                                    | デフォルト              |
| ------------------- | ----------------------------------------------------------------------- | ----------------------- |
| `DATABASE_URL`      | PostgreSQL 接続 URL                                                     | -                       |
| `POSTGRES_USER`     | DB ユーザー名                                                           | `t_rader`               |
| `POSTGRES_PASSWORD` | DB パスワード                                                           | `t_rader`               |
| `POSTGRES_DB`       | DB 名                                                                   | `t_rader_development`   |
| `DB_PORT`           | DB 公開ポート                                                           | `5432`                  |
| `BACKEND_PORT`      | バックエンド公開ポート                                                  | `3000`                  |
| `FRONTEND_PORT`     | フロントエンド公開ポート                                                | `5173`                  |
| `JQUANTS_API_KEY`   | J-Quants API キー (`DATA_PROVIDER=jquants` 時に使用)                    | -                       |
| `VITE_API_URL`      | Vite 開発サーバーのプロキシ先 URL                                       | `http://localhost:3000` |
| `API_BACKEND_URL`   | nginx リバースプロキシの転送先 URL (本番用、実行時に設定必須)           | -                       |
| `NGINX_RESOLVER`    | nginx の DNS リゾルバ (Kubernetes: kube-dns アドレス、実行時に設定必須) | -                       |
| `MCP_ALLOWED_HOSTS` | MCP server が受理する `Host` header の追加許可リスト (カンマ区切り)     | -                       |

### DataProvider 切替

`DATA_PROVIDER` 環境変数で価格データの取得元を選ぶ。デフォルト (未設定) は `jquants`。

| 値        | 必要な追加変数                                                                                 | 用途                                                            |
| --------- | ---------------------------------------------------------------------------------------------- | --------------------------------------------------------------- |
| `jquants` | `JQUANTS_API_KEY`                                                                              | J-Quants API (無料枠は 12 週遅延あり)                           |
| `ibkr`    | `IBKR_BASE_URL` (任意), `IBKR_SESSION_TOKEN` (任意), `IBKR_EXCHANGE` (任意、デフォルト `TSEJ`) | IBKR Client Portal Web API。Gateway を別途常駐させて URL を指す |
| `none`    | (なし)                                                                                         | DataProvider を無効化。データ取得系エンドポイントは 503 を返す  |

IBKR を使う場合は Client Portal Gateway を VKE クラスタ等に常駐させ、その HTTP エンドポイントを `IBKR_BASE_URL` に設定する (例: `https://ibkr-gateway:5000/v1/api`)。秘密鍵相当の API キーは存在せず、認証は Gateway 側の Web ログインで維持される。

## Deployment と外部連携

- [`docs/mcp.md`](./docs/mcp.md): `/mcp/mgmt` と `/mcp/strategy` の tool 一覧、session 永続化、`MCP_ALLOWED_HOSTS` の挙動
- [`docs/deployment.md`](./docs/deployment.md): 必須 env、backend が要求する権限、Service port
- [`docs/kubeopencode-integration.md`](./docs/kubeopencode-integration.md): kubeopencode operator との連携で backend が依存する seam
