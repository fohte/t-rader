use std::net::SocketAddr;
use std::sync::Arc;

use backend::cli::Cli;
use backend::{
    build_agent_webhook_state, build_external_webhook_state, build_http_state, create_router,
};
use clap::Parser;
use core_application::agent_task_client::{AgentTaskClient, SharedAgentTaskClient};
use core_application::calendar::source::SharedCalendarEventSource;
use core_application::daily_bar_source::SharedDailyBarSource;
use core_application::earnings_schedule_source::SharedEarningsScheduleSource;
use core_application::equity_master_source::SharedEquityMasterSource;
use core_application::financial_summary_source::SharedFinancialSummarySource;
use core_application::indicator_observation_source::SharedIndicatorObservationSource;
use core_application::kata_exec::{KataExecutor, SharedKataExecutor};
use core_application::llm_client::SharedLlmClient;
use core_application::margin_source::SharedMarginSource;
use core_application::market_daily_bar_source::SharedMarketDailyBarSource;
use core_application::news_aggregator::SharedNewsAggregator;
use core_application::shareholding_structure_source::SharedShareholdingStructureSource;
use core_application::short_selling_source::SharedShortSellingSource;
use core_application::valuation_source::SharedValuationSource;
use entrypoint_frontend_api::FrontendApiState;
use entrypoint_scheduler::{Scheduler, SchedulerDependencies};
use futures_util::future::BoxFuture;
use gateway_fred::FredClient;
use gateway_ibkr::{IbkrClient, RATE_LIMIT_KEY_PREFIX};
use gateway_jquants::JQuantsClient;
use gateway_kata_exec::{HttpKataExecutor, KataExecutorConfig};
use gateway_litellm::LiteLlmClient as LlmGatewayClient;
use gateway_postgres::{DatabaseHandle, PostgresIngestRunLog};
use gateway_rss::RssNewsAggregator;
use gateway_t_rader_agent::{
    AgentTaskClientConfig, AgentTaskClientConfigSource, HttpAgentTaskClient,
};
use migration::{Migrator, MigratorTrait};
use rate_limit::RateLimiter;
use sea_orm::{ConnectOptions, Database};
use tokio::sync::watch;

mod admin_ui;
mod logging;
mod runtime;
mod signals;
mod startup;

use logging::default_log_filter;
use signals::{wait_for_os_shutdown_signal, wait_for_shutdown};
use startup::{
    StartupError, jquants_config_from_env, required_redis_url, worker_admin_ui_settings_from_env,
};

#[tokio::main]
async fn main() -> Result<(), StartupError> {
    let cli = Cli::parse();

    // --dump-openapi: OpenAPI スペックを JSON で標準出力に出力して終了する
    if cli.dump_openapi {
        let spec = backend::create_openapi_spec();
        let json = spec
            .to_pretty_json()
            .map_err(|e| StartupError::Config(format!("failed to serialize OpenAPI spec: {e}")))?;
        println!("{json}");
        return Ok(());
    }

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| default_log_filter()),
        )
        .init();

    let jquants_config = if cli.migrate_only {
        None
    } else {
        jquants_config_from_env()?
    };

    let database_url = std::env::var("DATABASE_URL").map_err(|_| {
        StartupError::Config("DATABASE_URL environment variable is not set".to_string())
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
        backend::migrations::migrate_graphile_worker_schema(
            db.get_postgres_connection_pool().clone(),
        )
        .await
        .map_err(|error| {
            StartupError::Config(format!("failed to migrate Graphile Worker schema: {error}"))
        })?;
        tracing::info!("database migrations completed");
    } else {
        tracing::info!("skipping database migrations (--skip-migration)");
    }

    // --migrate-only: マイグレーションのみ実行して終了する
    if cli.migrate_only {
        tracing::info!("migration completed, exiting (--migrate-only)");
        return Ok(());
    }

    let admin_ui_settings = if cli.run_mode.starts_worker() {
        Some(worker_admin_ui_settings_from_env()?)
    } else {
        None
    };

    let redis_url = required_redis_url(std::env::var("REDIS_URL").ok())?;

    let app_db = DatabaseHandle::from(db.clone());
    let provider_kind = std::env::var("DATA_PROVIDER")
        .ok()
        .map(|s| s.to_lowercase())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "jquants".to_string());

    let (daily_bar_source, jquants_ingest_client) = match provider_kind.as_str() {
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
            let rate_limiter =
                RateLimiter::new(&redis_url, RATE_LIMIT_KEY_PREFIX).map_err(|e| {
                    StartupError::Config(format!("failed to initialize IBKR rate limiter: {e}"))
                })?;
            let client = Arc::new(
                IbkrClient::new(
                    base_url,
                    session_token,
                    exchange,
                    rate_limiter,
                    std::time::Duration::from_secs(30),
                )
                .map_err(|e| {
                    StartupError::Config(format!("failed to initialize IBKR client: {e}"))
                })?,
            );
            tracing::info!("IBKR 日足データ取得元を初期化しました");
            let source: SharedDailyBarSource = client;
            (Some(source), None)
        }
        "jquants" => match jquants_config {
            Some((api_key, plan)) => {
                let request_client = Arc::new(
                    JQuantsClient::new(
                        &redis_url,
                        api_key.clone(),
                        plan,
                        std::time::Duration::from_secs(30),
                    )
                    .map_err(|error| {
                        StartupError::Config(format!(
                            "failed to initialize J-Quants request client: {error}"
                        ))
                    })?,
                );
                // `Duration::ZERO` は quota 不足時に即時エラーになるため、取り込み側は最大値を渡す。
                let ingest_client = Arc::new(
                    JQuantsClient::new(&redis_url, api_key, plan, std::time::Duration::MAX)
                        .map_err(|error| {
                            StartupError::Config(format!(
                                "failed to initialize J-Quants ingest client: {error}"
                            ))
                        })?,
                );
                tracing::info!("J-Quants 日足データ取得元を初期化しました");
                let source: SharedDailyBarSource = request_client.clone();
                (Some(source), Some(ingest_client))
            }
            _ => {
                tracing::warn!("JQUANTS_API_KEY が未設定のため、日足データ取得元なしで起動します");
                (None, None)
            }
        },
        other => {
            return Err(StartupError::Config(format!(
                "unknown DATA_PROVIDER value: '{other}' (expected: jquants | ibkr | none)"
            )));
        }
    };

    // t-rader-agent 内部 API client。戦略タスクの投入 / 状態照会を担う。
    let agent_task_client_config =
        AgentTaskClientConfig::from_env().map_err(|e| StartupError::Config(e.to_string()))?;
    let strategy_task_reconcile_enabled = matches!(
        &agent_task_client_config,
        AgentTaskClientConfigSource::Configured(_)
    );
    let agent_task_client: SharedAgentTaskClient = match agent_task_client_config {
        AgentTaskClientConfigSource::Configured(config) => {
            let client = HttpAgentTaskClient::new(config).map_err(|e| {
                StartupError::Config(format!("failed to initialize agent task client: {e}"))
            })?;
            tracing::info!("agent task client initialized");
            let arc: Arc<dyn AgentTaskClient + Send + Sync> = Arc::new(client);
            arc
        }
        AgentTaskClientConfigSource::Disabled => {
            tracing::warn!(
                "TRADER_AGENT_API_URL=disabled: agent task client を無効化して起動します (dev 用 opt-out)"
            );
            FrontendApiState::disabled_agent_task_client()
        }
    };

    // フィード一覧を取り込みごとに読み直し、UI / MCP からの変更を反映する。
    let news_aggregator: SharedNewsAggregator =
        Arc::new(RssNewsAggregator::new(&redis_url).map_err(|err| {
            StartupError::Config(format!("failed to initialize RSS news aggregator: {err}"))
        })?);
    let use_cases = backend::services::use_cases::build_use_cases(db.clone());

    let (fred_source, fred_calendar_event_source): (
        Option<SharedIndicatorObservationSource>,
        Option<SharedCalendarEventSource>,
    ) = match std::env::var("FRED_API_KEY") {
        Ok(api_key) if !api_key.is_empty() => {
            let fred_client = Arc::new(FredClient::new(api_key).map_err(|err| {
                StartupError::Config(format!("failed to initialize FRED client: {err}"))
            })?);
            let fred_source: SharedIndicatorObservationSource = fred_client.clone();
            let fred_calendar_event_source: SharedCalendarEventSource = fred_client;
            (Some(fred_source), Some(fred_calendar_event_source))
        }
        _ => {
            tracing::warn!("FRED_API_KEY が未設定のため、FRED の取り込みを起動しません");
            (None, None)
        }
    };

    let short_selling_source: Option<SharedShortSellingSource> = jquants_ingest_client
        .as_ref()
        .map(|client| Arc::clone(client) as SharedShortSellingSource);
    let margin_source: Option<SharedMarginSource> = jquants_ingest_client
        .as_ref()
        .map(|client| Arc::clone(client) as SharedMarginSource);
    let market_daily_bar_source: Option<SharedMarketDailyBarSource> = jquants_ingest_client
        .as_ref()
        .map(|client| Arc::clone(client) as SharedMarketDailyBarSource);
    let earnings_schedule_source: Option<SharedEarningsScheduleSource> = jquants_ingest_client
        .as_ref()
        .map(|client| Arc::clone(client) as SharedEarningsScheduleSource);
    let financial_summary_source: Option<SharedFinancialSummarySource> = jquants_ingest_client
        .as_ref()
        .map(|client| Arc::clone(client) as SharedFinancialSummarySource);
    let equity_master_source: Option<SharedEquityMasterSource> = jquants_ingest_client
        .as_ref()
        .map(|client| Arc::clone(client) as SharedEquityMasterSource);
    let shareholding_structure_source: Option<SharedShareholdingStructureSource> =
        jquants_ingest_client
            .as_ref()
            .map(|client| Arc::clone(client) as SharedShareholdingStructureSource);
    let valuation_source: Option<SharedValuationSource> = jquants_ingest_client
        .as_ref()
        .map(|client| Arc::clone(client) as SharedValuationSource);
    let dependencies = SchedulerDependencies {
        bars: use_cases.bars(),
        market_daily_bar_source,
        news: use_cases.news(),
        news_aggregator,
        earnings_schedules: use_cases.earnings_schedules(),
        earnings_schedule_source,
        financial_summaries: use_cases.financial_summaries(),
        financial_summary_source,
        equity_master: use_cases.equity_master(),
        equity_master_source,
        shareholding_structures: use_cases.shareholding_structures(),
        shareholding_structure_source,
        valuations: use_cases.valuations(),
        valuation_source,
        indicator_observations: use_cases.indicator_observations(),
        calendar_events: use_cases.calendar_events(),
        ingest_run_log: Arc::new(PostgresIngestRunLog::new(app_db.clone())),
        fred_source,
        fred_calendar_event_source,
        predictions: use_cases.predictions(),
        short_ratios: use_cases.short_ratios(),
        short_sale_reports: use_cases.short_sale_reports(),
        margins: use_cases.margins(),
        short_selling_source,
        margin_source,
        strategy_tasks: use_cases.strategy_tasks(),
        triggers: use_cases.triggers(),
        agent_task_client: agent_task_client.clone(),
        strategy_task_reconcile_enabled,
    };
    let app = if cli.run_mode.starts_api() {
        // agent_task_client が disabled でも opt-out させない。空文字を webhook token の
        // デフォルトにすると、通知ハンドラがヘッダ未設定時に空文字へフォールバックする実装と
        // 合わさって空文字同士の一致で認証をすり抜けてしまう。
        let agent_webhook_token = std::env::var("AGENT_WEBHOOK_TOKEN")
            .ok()
            .filter(|s| !s.is_empty())
            .ok_or_else(|| {
                StartupError::Config(
                    "AGENT_WEBHOOK_TOKEN environment variable is not set".to_string(),
                )
            })?;

        let kata_executor: Option<SharedKataExecutor> = match KataExecutorConfig::from_env() {
            Some(config) => match HttpKataExecutor::new(config) {
                Ok(executor) => {
                    tracing::info!("kata executor initialized");
                    let arc: Arc<dyn KataExecutor + Send + Sync> = Arc::new(executor);
                    Some(arc)
                }
                Err(e) => {
                    return Err(StartupError::Config(format!(
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

        let llm_gateway_client =
            LlmGatewayClient::from_env().map(|client| Arc::new(client) as SharedLlmClient);
        let agent_webhook_state = build_agent_webhook_state(&use_cases, agent_webhook_token);
        let external_webhook_state =
            build_external_webhook_state(&use_cases, agent_task_client.clone());
        let state = build_http_state(
            &use_cases,
            agent_task_client,
            kata_executor,
            llm_gateway_client,
        );
        Some(create_router(
            state,
            agent_webhook_state,
            external_webhook_state,
            use_cases,
            daily_bar_source,
            app_db,
        ))
    } else {
        None
    };

    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let signal_tx = shutdown_tx.clone();
    let _shutdown_listener = tokio::spawn(async move {
        if let Err(error) = wait_for_os_shutdown_signal().await {
            tracing::error!(%error, "failed to listen for shutdown signal");
        }
        let _ = signal_tx.send(true);
    });

    let worker = if cli.run_mode.starts_worker() {
        let worker = Scheduler::initialize(
            db.get_postgres_connection_pool().clone(),
            dependencies,
            wait_for_shutdown(shutdown_rx.clone()),
        )
        .await
        .map_err(|error| {
            StartupError::Config(format!("failed to initialize Graphile Worker: {error}"))
        })?;
        tracing::info!("Graphile Worker initialized");
        Some(worker)
    } else {
        None
    };

    let admin_server_run = if let Some(settings) = admin_ui_settings {
        Some(admin_ui::server(settings, &db, shutdown_rx.clone()).await?)
    } else {
        None
    };

    let server_run: Option<BoxFuture<'static, Result<(), std::io::Error>>> = if let Some(app) = app
    {
        let port: u16 = std::env::var("BACKEND_PORT")
            .ok()
            .and_then(|p| p.parse().ok())
            .unwrap_or(3000);
        let addr = SocketAddr::from(([0, 0, 0, 0], port));
        tracing::info!("listening on {addr}");

        let listener = tokio::net::TcpListener::bind(addr)
            .await
            .map_err(|e| StartupError::Config(format!("failed to bind to {addr}: {e}")))?;
        let server_shutdown = wait_for_shutdown(shutdown_rx);
        Some(Box::pin(async move {
            axum::serve(
                listener,
                app.into_make_service_with_connect_info::<SocketAddr>(),
            )
            .with_graceful_shutdown(server_shutdown)
            .await
        }))
    } else {
        None
    };

    let mut run_futures: Vec<BoxFuture<'static, Result<(), StartupError>>> = Vec::new();
    if let Some(worker) = worker {
        run_futures.push(Box::pin(async move {
            worker
                .run()
                .await
                .map_err(|error| StartupError::Runtime(format!("Graphile Worker failed: {error}")))
        }));
    }
    if let Some(server_run) = server_run {
        run_futures.push(Box::pin(async move {
            server_run
                .await
                .map_err(|error| StartupError::Runtime(format!("server error: {error}")))
        }));
    }
    if let Some(admin_server_run) = admin_server_run {
        run_futures.push(admin_server_run);
    }
    runtime::supervise(run_futures, shutdown_tx).await
}
