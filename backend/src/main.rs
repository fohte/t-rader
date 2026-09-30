use std::net::SocketAddr;
use std::sync::Arc;

use backend::AppState;
use backend::agent_client::{
    AgentTaskClient, AgentTaskClientConfig, AgentTaskClientConfigSource, HttpAgentTaskClient,
    SharedAgentTaskClient,
};
use backend::cli::Cli;
use backend::create_router;
use backend::data_provider::SharedDailyBarSource;
use backend::data_provider::news::rss::RssNewsAggregator;
use backend::error::AppError;
use backend::kata_exec::{HttpKataExecutor, KataExecutor, KataExecutorConfig, SharedKataExecutor};
use backend::services::litellm_client::{LiteLlmClient as LlmGatewayClient, SharedLlmClient};
use clap::Parser;
use core_application::indicator_observation_source::SharedIndicatorObservationSource;
use core_application::news_aggregator::SharedNewsAggregator;
use gateway_fred::FredClient;
use gateway_ibkr::IbkrClient;
use gateway_jquants::{JQuantsClient, JQuantsPlan};
use gateway_postgres::DatabaseHandle;
use migration::{Migrator, MigratorTrait};
use sea_orm::{ConnectOptions, Database};

fn parse_jquants_plan(value: Option<String>) -> Result<JQuantsPlan, AppError> {
    let value = value.filter(|value| !value.is_empty()).ok_or_else(|| {
        AppError::Config("JQUANTS_PLAN is required when JQUANTS_API_KEY is configured".to_string())
    })?;
    value
        .parse()
        .map_err(|error| AppError::Config(format!("invalid JQUANTS_PLAN value '{value}': {error}")))
}

fn jquants_config(
    api_key: Option<String>,
    plan: Option<String>,
) -> Result<Option<(String, JQuantsPlan)>, AppError> {
    match api_key.filter(|api_key| !api_key.is_empty()) {
        Some(api_key) => Ok(Some((api_key, parse_jquants_plan(plan)?))),
        None => Ok(None),
    }
}

fn jquants_config_from_env() -> Result<Option<(String, JQuantsPlan)>, AppError> {
    jquants_config(
        std::env::var("JQUANTS_API_KEY").ok(),
        std::env::var("JQUANTS_PLAN").ok(),
    )
}

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

    let jquants_config = if cli.migrate_only {
        None
    } else {
        jquants_config_from_env()?
    };

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

    let app_db = DatabaseHandle::from(db.clone());
    let use_cases = backend::services::use_cases::build_use_cases(app_db.clone());

    let provider_kind = std::env::var("DATA_PROVIDER")
        .ok()
        .map(|s| s.to_lowercase())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "jquants".to_string());

    let (daily_bar_source, jquants_client) = match provider_kind.as_str() {
        "none" => {
            tracing::info!("DATA_PROVIDER=none: 日足データの取得元を無効化して起動します");
            (None, None)
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
            let client = Arc::new(
                IbkrClient::new(base_url, session_token, exchange).map_err(|e| {
                    AppError::Config(format!("failed to initialize IBKR client: {e}"))
                })?,
            );
            tracing::info!("IBKR 日足データ取得元を初期化しました");
            let source: SharedDailyBarSource = client;
            (Some(source), None)
        }
        "jquants" => match jquants_config {
            Some((api_key, plan)) => {
                let client = Arc::new(JQuantsClient::new(api_key, plan).map_err(|error| {
                    AppError::Config(format!("failed to initialize J-Quants client: {error}"))
                })?);
                tracing::info!("J-Quants 日足データ取得元を初期化しました");
                let source: SharedDailyBarSource = client.clone();
                (Some(source), Some(client))
            }
            _ => {
                tracing::warn!("JQUANTS_API_KEY が未設定のため、日足データ取得元なしで起動します");
                (None, None)
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

    let fred_source: Option<SharedIndicatorObservationSource> = match std::env::var("FRED_API_KEY")
    {
        Ok(api_key) if !api_key.is_empty() => {
            let fred_client = FredClient::new(api_key).map_err(|err| {
                AppError::Config(format!("failed to initialize FRED client: {err}"))
            })?;
            let source: SharedIndicatorObservationSource = Arc::new(fred_client);
            Some(source)
        }
        _ => {
            tracing::warn!(
                "FRED_API_KEY が未設定のため、FRED マクロ指標履歴の取り込みを起動しません"
            );
            None
        }
    };

    // cron trigger を schedule どおりに発火させる worker を起動する。
    // 戻り値は意図的に捨てる: ランタイム終了で task ごと止まる。
    tracing::info!(
        interval_secs = backend::services::trigger_worker::DEFAULT_INTERVAL.as_secs(),
        "starting cron trigger worker",
    );
    let _trigger_worker = backend::services::trigger_worker::spawn(
        use_cases.triggers(),
        agent_task_client.clone(),
        backend::services::trigger_worker::DEFAULT_INTERVAL,
    );

    if let Some(client) = &jquants_client {
        let _daily_bars_ingest_poll = backend::services::daily_bars_ingest::spawn_poll(
            db.clone(),
            client.clone(),
            backend::services::daily_bars_ingest::DEFAULT_INTERVAL,
        );
        tracing::info!(
            interval_secs = backend::services::daily_bars_ingest::DEFAULT_INTERVAL.as_secs(),
            "daily bars ingest poll task started",
        );
    }

    let llm_gateway_client =
        LlmGatewayClient::from_env().map(|client| Arc::new(client) as SharedLlmClient);

    let worker = backend::entrypoints::scheduler::initialize(
        db.clone(),
        use_cases.clone(),
        fred_source,
        jquants_client.clone(),
    )
    .await
    .map_err(|error| AppError::Config(format!("failed to initialize Graphile Worker: {error}")))?;
    tracing::info!("Graphile Worker initialized");
    let state = AppState {
        db: app_db,
        use_cases,
        daily_bar_source,
        jquants_client,
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

    tokio::select! {
        result = worker.run() => result
            .map_err(|error| AppError::Config(format!("Graphile Worker failed: {error}"))),
        result = axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        ) => result.map_err(|error| AppError::Config(format!("server error: {error}"))),
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case::api_key_missing(None, Some("invalid".to_string()), Ok(None))]
    #[case::api_key_empty(Some(String::new()), Some("invalid".to_string()), Ok(None))]
    #[case::plan_missing(Some("test-api-key".to_string()), None, Err(()))]
    #[case::plan_empty(Some("test-api-key".to_string()), Some(String::new()), Err(()))]
    #[case::plan_invalid(
        Some("test-api-key".to_string()),
        Some("enterprise".to_string()),
        Err(()),
    )]
    #[case::plan_valid(
        Some("test-api-key".to_string()),
        Some("standard".to_string()),
        Ok(Some(("test-api-key".to_string(), JQuantsPlan::Standard))),
    )]
    fn test_jquants_config(
        #[case] api_key: Option<String>,
        #[case] plan: Option<String>,
        #[case] expected: Result<Option<(String, JQuantsPlan)>, ()>,
    ) {
        assert_eq!(jquants_config(api_key, plan).map_err(|_| ()), expected);
    }
}
