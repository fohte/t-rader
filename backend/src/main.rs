use std::net::SocketAddr;
use std::sync::Arc;

use backend::AppState;
use backend::agent_client::{
    AgentTaskClient, AgentTaskClientConfig, AgentTaskClientConfigSource, HttpAgentTaskClient,
    SharedAgentTaskClient,
};
use backend::cli::Cli;
use backend::create_router;
use backend::data_provider::DataProviderKind;
use backend::data_provider::ibkr::IbkrClient;
use backend::data_provider::jquants::JQuantsClient;
use backend::data_provider::news::rss::RssNewsAggregator;
use backend::error::AppError;
use backend::kata_exec::{HttpKataExecutor, KataExecutor, KataExecutorConfig, SharedKataExecutor};
use backend::services::litellm_client::{LiteLlmClient as LlmGatewayClient, SharedLlmClient};
use clap::Parser;
use core_application::{IndicatorObservationSource, SharedNewsAggregator};
use gateway_fred::FredClient;
use migration::{Migrator, MigratorTrait};
use sea_orm::{ConnectOptions, Database};

#[tokio::main]
async fn main() -> Result<(), AppError> {
    let cli = Cli::parse();

    // --dump-openapi: OpenAPI スペックを JSON で標準出力に出力して終了する
    if cli.dump_openapi {
        let spec = backend::create_openapi_spec();
        let json = spec
            .to_pretty_json()
            .map_err(|e| AppError::Config(format!("failed to serialize OpenAPI spec: {e}")))?;
        println!("{json}");
        return Ok(());
    }

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let database_url = std::env::var("DATABASE_URL").map_err(|_| {
        AppError::Config("DATABASE_URL environment variable is not set".to_string())
    })?;

    let mut opt = ConnectOptions::new(&database_url);
    opt.max_connections(5);
    // insert_many は行数ごとに distinct な SQL になり、キャッシュに積み続けると OOM する。
    // capacity 0 だと evict 時の Close が送られず prepared statement が PG 側に残り続けるため、
    // 1 にして evict のたびに Close させる
    opt.map_sqlx_postgres_opts(|opts| opts.statement_cache_capacity(1));

    let db = Database::connect(opt).await?;

    // --skip-migration が指定されていない場合のみマイグレーションを実行する
    if !cli.skip_migration {
        tracing::info!("running database migrations");
        Migrator::up(&db, None).await?;
        tracing::info!("database migrations completed");
    } else {
        tracing::info!("skipping database migrations (--skip-migration)");
    }

    // --migrate-only: マイグレーションのみ実行して終了する
    if cli.migrate_only {
        tracing::info!("migration completed, exiting (--migrate-only)");
        return Ok(());
    }

    let provider_kind = std::env::var("DATA_PROVIDER")
        .ok()
        .map(|s| s.to_lowercase())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "jquants".to_string());

    let data_provider = match provider_kind.as_str() {
        "none" => {
            tracing::info!("DATA_PROVIDER=none: DataProvider を無効化して起動します");
            None
        }
        "ibkr" => {
            let base_url = std::env::var("IBKR_BASE_URL")
                .ok()
                .filter(|s| !s.is_empty());
            let session_token = std::env::var("IBKR_SESSION_TOKEN")
                .ok()
                .filter(|s| !s.is_empty());
            let exchange = std::env::var("IBKR_EXCHANGE")
                .ok()
                .filter(|s| !s.is_empty());
            let client = IbkrClient::new(base_url, session_token, exchange)?;
            tracing::info!("IBKR DataProvider を初期化しました");
            Some(Arc::new(DataProviderKind::Ibkr(client)))
        }
        "jquants" => match std::env::var("JQUANTS_API_KEY") {
            Ok(api_key) if !api_key.is_empty() => {
                let client = JQuantsClient::new(api_key)?;
                let manual_plan =
                    backend::services::jquants_plan_setting::find_current(&db)
                        .await?
                        .map(|row| {
                            backend::models::parse_plan_setting::<
                                backend::models::JQuantsPlanSettingData,
                            >(row.plan_setting)
                        })
                        .transpose()?
                        .and_then(|data| data.plan);
                client.set_manual_plan(manual_plan);
                tracing::info!("J-Quants DataProvider を初期化しました");
                Some(Arc::new(DataProviderKind::JQuants(client)))
            }
            _ => {
                tracing::warn!("JQUANTS_API_KEY が未設定のため、DataProvider なしで起動します");
                None
            }
        },
        other => {
            return Err(AppError::Config(format!(
                "unknown DATA_PROVIDER value: '{other}' (expected: jquants | ibkr | none)"
            )));
        }
    };

    // t-rader-agent 内部 API client。戦略タスクの投入 / 状態照会を担う。webhook 受信時の
    // 即時 polling 誘発用に Notify を watcher と共有する。
    let agent_task_notify = Arc::new(tokio::sync::Notify::new());
    let agent_task_client: SharedAgentTaskClient = match AgentTaskClientConfig::from_env()
        .map_err(|e| AppError::Config(e.to_string()))?
    {
        AgentTaskClientConfigSource::Configured(config) => {
            let client = HttpAgentTaskClient::new(config).map_err(|e| {
                AppError::Config(format!("failed to initialize agent task client: {e}"))
            })?;
            tracing::info!("agent task client initialized");
            let arc: Arc<dyn AgentTaskClient + Send + Sync> = Arc::new(client);
            let _watcher = backend::mcp::watcher::spawn(
                db.clone(),
                arc.clone(),
                backend::mcp::watcher::DEFAULT_INTERVAL,
                agent_task_notify.clone(),
            );
            arc
        }
        AgentTaskClientConfigSource::Disabled => {
            tracing::warn!(
                "TRADER_AGENT_API_URL=disabled: agent task client を無効化して起動します (dev 用 opt-out)"
            );
            AppState::disabled_agent_task_client()
        }
    };

    // agent_task_client が disabled でも opt-out させない。空文字を webhook token の
    // デフォルトにすると、通知ハンドラがヘッダ未設定時に空文字へフォールバックする実装と
    // 合わさって空文字同士の一致で認証をすり抜けてしまう。
    let agent_webhook_token = std::env::var("AGENT_WEBHOOK_TOKEN")
        .ok()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| {
            AppError::Config("AGENT_WEBHOOK_TOKEN environment variable is not set".to_string())
        })?;

    let kata_executor: Option<SharedKataExecutor> = match KataExecutorConfig::from_env() {
        Some(config) => match HttpKataExecutor::new(config) {
            Ok(executor) => {
                tracing::info!("kata executor initialized");
                let arc: Arc<dyn KataExecutor + Send + Sync> = Arc::new(executor);
                Some(arc)
            }
            Err(e) => {
                return Err(AppError::Config(format!(
                    "failed to initialize kata executor: {e}"
                )));
            }
        },
        None => {
            tracing::warn!(
                "KATA_EXEC_API_URL が未設定のため、kata executor を無効化して起動します"
            );
            None
        }
    };

    // 公開 RSS から 1h 間隔でニュースを集約する poll task を起動する。
    // フィード一覧は `rss_feed` テーブルから tick ごとに読み直す (UI / MCP からの追加・無効化を
    // 再起動なしで反映するため)。0 件運用も許容する。
    let news_aggregator: SharedNewsAggregator =
        Arc::new(RssNewsAggregator::new().map_err(|err| {
            AppError::Config(format!("failed to initialize RSS news aggregator: {err}"))
        })?);
    let _news_poll = backend::services::news::spawn_poll(
        db.clone(),
        news_aggregator,
        std::time::Duration::from_secs(3600),
    );
    tracing::info!("news aggregation poll task started (public RSS, interval=1h)");

    let _prediction_grading_poll = backend::services::prediction_grading::spawn_poll(
        db.clone(),
        backend::services::prediction_grading::DEFAULT_INTERVAL,
    );
    tracing::info!(
        interval_secs = backend::services::prediction_grading::DEFAULT_INTERVAL.as_secs(),
        "prediction grading poll task started",
    );

    match std::env::var("FRED_API_KEY") {
        Ok(api_key) if !api_key.is_empty() => {
            let fred_client = FredClient::new(api_key).map_err(|err| {
                AppError::Config(format!("failed to initialize FRED client: {err}"))
            })?;
            let source: Arc<dyn IndicatorObservationSource> = Arc::new(fred_client);
            let _fred_ingest_poll = backend::services::fred_ingest::spawn_poll(
                db.clone(),
                source,
                backend::services::fred_ingest::DEFAULT_INTERVAL,
            );
            tracing::info!(
                interval_secs = backend::services::fred_ingest::DEFAULT_INTERVAL.as_secs(),
                "FRED macro history ingest poll task started",
            );
        }
        _ => {
            tracing::warn!(
                "FRED_API_KEY が未設定のため、FRED マクロ指標履歴の取り込みを起動しません"
            );
        }
    }

    // cron trigger を schedule どおりに発火させる worker を起動する。
    // 戻り値は意図的に捨てる: ランタイム終了で task ごと止まる。
    tracing::info!(
        interval_secs = backend::services::trigger_worker::DEFAULT_INTERVAL.as_secs(),
        "starting cron trigger worker",
    );
    let _trigger_worker = backend::services::trigger_worker::spawn(
        db.clone(),
        agent_task_client.clone(),
        backend::services::trigger_worker::DEFAULT_INTERVAL,
    );

    // IBKR provider には業種・財務情報に対応するデータが無いため対象外。
    if let Some(provider) = &data_provider
        && matches!(provider.as_ref(), DataProviderKind::JQuants(_))
    {
        let _stock_master_sync_poll = backend::services::stock_master_sync::spawn_poll(
            db.clone(),
            provider.clone(),
            backend::services::stock_master_sync::DEFAULT_INTERVAL,
        );
        tracing::info!(
            interval_secs = backend::services::stock_master_sync::DEFAULT_INTERVAL.as_secs(),
            "stock master sync poll task started",
        );

        let _short_sale_report_ingest_poll =
            backend::services::short_sale_report_ingest::spawn_poll(
                db.clone(),
                provider.clone(),
                backend::services::short_sale_report_ingest::DEFAULT_INTERVAL,
            );
        tracing::info!(
            interval_secs = backend::services::short_sale_report_ingest::DEFAULT_INTERVAL.as_secs(),
            "short sale report ingest poll task started",
        );

        let _short_ratio_ingest_poll = backend::services::short_ratio_ingest::spawn_poll(
            db.clone(),
            provider.clone(),
            backend::services::short_ratio_ingest::DEFAULT_INTERVAL,
        );
        tracing::info!(
            interval_secs = backend::services::short_ratio_ingest::DEFAULT_INTERVAL.as_secs(),
            "short ratio ingest poll task started",
        );

        let _margin_ingest_poll = backend::services::margin_ingest::spawn_poll(
            db.clone(),
            provider.clone(),
            backend::services::margin_ingest::DEFAULT_INTERVAL,
        );
        tracing::info!(
            interval_secs = backend::services::margin_ingest::DEFAULT_INTERVAL.as_secs(),
            "margin ingest poll task started",
        );

        let _fin_summary_ingest_poll = backend::services::fin_summary_ingest::spawn_poll(
            db.clone(),
            provider.clone(),
            backend::services::fin_summary_ingest::DEFAULT_INTERVAL,
        );
        tracing::info!(
            interval_secs = backend::services::fin_summary_ingest::DEFAULT_INTERVAL.as_secs(),
            "fin summary ingest poll task started",
        );

        let _earnings_date_ingest_poll = backend::services::earnings_date_ingest::spawn_poll(
            db.clone(),
            provider.clone(),
            backend::services::earnings_date_ingest::DEFAULT_INTERVAL,
        );
        tracing::info!(
            interval_secs = backend::services::earnings_date_ingest::DEFAULT_INTERVAL.as_secs(),
            "earnings date ingest poll task started",
        );

        let _edinet_holdings_poll = backend::services::edinet_holdings::spawn_poll(
            db.clone(),
            provider.clone(),
            backend::services::edinet_holdings::DEFAULT_INTERVAL,
        );
        tracing::info!(
            interval_secs = backend::services::edinet_holdings::DEFAULT_INTERVAL.as_secs(),
            "EDINET holdings ingest poll task started",
        );

        let _daily_bars_ingest_poll = backend::services::daily_bars_ingest::spawn_poll(
            db.clone(),
            provider.clone(),
            backend::services::daily_bars_ingest::DEFAULT_INTERVAL,
        );
        tracing::info!(
            interval_secs = backend::services::daily_bars_ingest::DEFAULT_INTERVAL.as_secs(),
            "daily bars ingest poll task started",
        );

        let _valuation_ingest_poll = backend::services::valuation_ingest::spawn_poll(
            db.clone(),
            provider.clone(),
            backend::services::valuation_ingest::DEFAULT_INTERVAL,
        );
        tracing::info!(
            interval_secs = backend::services::valuation_ingest::DEFAULT_INTERVAL.as_secs(),
            "valuation ingest poll task started",
        );
    }

    let llm_gateway_client =
        LlmGatewayClient::from_env().map(|client| Arc::new(client) as SharedLlmClient);

    let state = AppState {
        db,
        data_provider,
        agent_task_client,
        agent_task_notify,
        agent_webhook_token: Arc::from(agent_webhook_token),
        kata_executor,
        llm_gateway_client,
    };

    let app = create_router(state);

    let port: u16 = std::env::var("BACKEND_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(3000);
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    tracing::info!("listening on {addr}");

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .map_err(|e| AppError::Config(format!("failed to bind to {addr}: {e}")))?;

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .map_err(|e| AppError::Config(format!("server error: {e}")))?;

    Ok(())
}
