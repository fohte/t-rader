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

# 全サービスを起動 (db, backend, frontend)
docker compose up

# frontend のみ起動
docker compose up frontend
```

起動後、http://localhost:5173 でフロントエンドにアクセスできる。

## プロジェクト構成

```
├── frontend/          # React SPA
│   ├── src/
│   │   ├── components/  # UI コンポーネント
│   │   ├── routes/      # TanStack Router のファイルベースルーティング
│   │   └── main.tsx     # エントリーポイント
│   └── package.json
├── backend/           # Rust Axum サーバー
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
