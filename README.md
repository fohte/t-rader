# t-rader

個人向け日本株投資プラットフォーム

## セットアップ

```bash
cp .env.example .env
# .env を環境に合わせて編集
```

## 開発環境の起動

```bash
docker compose up
```

DB (TimescaleDB), backend (Rust/Axum), frontend (Vite/React) が起動する。

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
