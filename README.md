# t-rader

個人向け日本株投資プラットフォーム

## 技術スタック

| レイヤー               | 技術                                                          |
| ---------------------- | ------------------------------------------------------------- |
| Frontend               | React 19, Vite 7, TanStack Router, shadcn/ui, Tailwind CSS v4 |
| Backend                | Rust (Axum)                                                   |
| DB                     | TimescaleDB (PostgreSQL 17)                                   |
| パッケージマネージャー | Bun                                                           |
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

# 全サービスを起動 (db, backend, frontend)
docker compose up

# frontend のみ起動
docker compose up frontend
```

起動後、http://localhost:5173 でフロントエンドにアクセスできる。

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
docker compose exec db psql -U t_rader -d t_rader_development -c '\dt'

# hypertable の確認
docker compose exec db psql -U t_rader -d t_rader_development \
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
├── docker-compose.yml
└── .mise.toml         # ツールバージョン管理
```

## npm スクリプト (frontend/)

```bash
bun run dev        # Vite 開発サーバー
bun run build      # プロダクションビルド
bun run test       # 型チェック + ユニットテスト
bun run lint       # ESLint
bun run format     # ESLint + Prettier によるフォーマット
```

## 環境変数

| 変数                    | 説明                              | デフォルト            |
| ----------------------- | --------------------------------- | --------------------- |
| `DATABASE_URL`          | PostgreSQL 接続 URL               | -                     |
| `POSTGRES_USER`         | DB ユーザー名                     | `t_rader`             |
| `POSTGRES_PASSWORD`     | DB パスワード                     | `t_rader`             |
| `POSTGRES_DB`           | DB 名                             | `t_rader_development` |
| `DB_PORT`               | DB 公開ポート                     | `5432`                |
| `BACKEND_PORT`          | バックエンド公開ポート            | `3000`                |
| `FRONTEND_PORT`         | フロントエンド公開ポート          | `5173`                |
| `JQUANTS_REFRESH_TOKEN` | J-Quants API リフレッシュトークン | -                     |
