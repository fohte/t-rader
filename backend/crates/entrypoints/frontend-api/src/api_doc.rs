use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    tags(
        (name = "health", description = "ヘルスチェック"),
        (name = "bars", description = "バーデータ (OHLCV)"),
        (name = "calendar", description = "週間イベントカレンダー"),
        (name = "strategies", description = "戦略 (ワークスペース)"),
        (name = "agent_config", description = "目的 (purpose) 別の agent 設定 (AGENTS.md / skills / agent_graph)"),
        (name = "refs", description = "一級参照型 (stock / indicator / group)"),
        (name = "notes", description = "ノート"),
        (name = "note_kinds", description = "ノート種別"),
        (name = "annotations", description = "アノテーション"),
        (name = "comments", description = "コメントスレッド"),
        (name = "history", description = "変更履歴"),
        (name = "trades", description = "取引履歴と損益サマリ"),
        (name = "tasks", description = "戦略タスクの実行履歴 (口座横断)"),
        (name = "triggers", description = "戦略 trigger (cron / hook)"),
        (name = "paper_accounts", description = "ペーパートレード口座"),
        (name = "imports", description = "外部ソースからの取込 (SBI CSV 等)"),
        (name = "custom_indicators", description = "カスタムインジケーター (Python 定義)"),
        (name = "group_axes", description = "銘柄を分類する軸"),
        (name = "rss_feeds", description = "ニュース集約対象の RSS フィード定義"),
        (name = "ingest_status", description = "取り込み job の状態"),
        (name = "agent_options", description = "戦略 Agent 設定フォームの選択肢 (モデル一覧・tool 一覧)"),
        (name = "config", description = "frontend 向けランタイム設定値"),
        (name = "account", description = "口座全体の設定"),
    ),
    info(
        title = "T-Rader API",
        version = "0.1.0",
        description = "日本株投資プラットフォーム T-Rader の API",
    ),
)]
pub(crate) struct ApiDoc;
