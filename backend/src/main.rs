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
use core_application::earnings_schedule_source::SharedEarningsScheduleSource;
use core_application::equity_master_source::SharedEquityMasterSource;
use core_application::financial_summary_source::SharedFinancialSummarySource;
use core_application::indicator_observation_source::SharedIndicatorObservationSource;
use core_application::margin_source::SharedMarginSource;
use core_application::market_daily_bar_source::SharedMarketDailyBarSource;
use core_application::news_aggregator::SharedNewsAggregator;
use core_application::shareholding_structure_source::SharedShareholdingStructureSource;
use core_application::short_selling_source::SharedShortSellingSource;
use core_application::valuation_source::SharedValuationSource;
use entrypoint_scheduler::{Scheduler, SchedulerDependencies};
use futures_util::future::BoxFuture;
use gateway_fred::FredClient;
use gateway_ibkr::{IbkrClient, RATE_LIMIT_KEY_PREFIX};
use gateway_jquants::{JQuantsClient, JQuantsPlan};
use gateway_postgres::{DatabaseHandle, PostgresIngestRunLog};
use migration::{Migrator, MigratorTrait};
use rate_limit::RateLimiter;
use sea_orm::{ConnectOptions, Database};
use tokio::sync::watch;

const DEFAULT_LOG_FILTER: &str = "info,sqlx=warn";

fn default_log_filter() -> tracing_subscriber::EnvFilter {
    tracing_subscriber::EnvFilter::new(DEFAULT_LOG_FILTER)
}

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

async fn wait_for_shutdown(mut receiver: watch::Receiver<bool>) {
    let _ = receiver.wait_for(|shutdown| *shutdown).await;
}

async fn wait_for_os_shutdown_signal() -> Result<(), std::io::Error> {
    let ctrl_c = tokio::signal::ctrl_c();

    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};

        let mut terminate = signal(SignalKind::terminate())?;
        tokio::select! {
            result = ctrl_c => result,
            _ = terminate.recv() => Ok(()),
        }
    }

    #[cfg(not(unix))]
    ctrl_c.await
}

fn required_redis_url(value: Option<String>) -> Result<String, AppError> {
    value
        .filter(|value| !value.is_empty())
        .ok_or_else(|| AppError::Config("REDIS_URL environment variable is not set".to_string()))
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
                .unwrap_or_else(|_| default_log_filter()),
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
        backend::migrations::migrate_graphile_worker_schema(
            db.get_postgres_connection_pool().clone(),
        )
        .await
        .map_err(|error| {
            AppError::Config(format!("failed to migrate Graphile Worker schema: {error}"))
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

    let redis_url = required_redis_url(std::env::var("REDIS_URL").ok())?;

    let app_db = DatabaseHandle::from(db.clone());
    let provider_kind = std::env::var("DATA_PROVIDER")
        .ok()
        .map(|s| s.to_lowercase())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "jquants".to_string());

    let (daily_bar_source, jquants_client, jquants_ingest_client) = match provider_kind.as_str() {
        "none" => {
            tracing::info!("DATA_PROVIDER=none: 日足データの取得元を無効化して起動します");
            (None, None, None)
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
                    AppError::Config(format!("failed to initialize IBKR rate limiter: {e}"))
                })?;
            let client = Arc::new(
                IbkrClient::new(
                    base_url,
                    session_token,
                    exchange,
                    rate_limiter,
                    std::time::Duration::from_secs(30),
                )
                .map_err(|e| AppError::Config(format!("failed to initialize IBKR client: {e}")))?,
            );
            tracing::info!("IBKR 日足データ取得元を初期化しました");
            let source: SharedDailyBarSource = client;
            (Some(source), None, None)
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
                        AppError::Config(format!(
                            "failed to initialize J-Quants request client: {error}"
                        ))
                    })?,
                );
                // `Duration::ZERO` は quota 不足時に即時エラーになるため、取り込み側は最大値を渡す。
                let ingest_client = Arc::new(
                    JQuantsClient::new(&redis_url, api_key, plan, std::time::Duration::MAX)
                        .map_err(|error| {
                            AppError::Config(format!(
                                "failed to initialize J-Quants ingest client: {error}"
                            ))
                        })?,
                );
                tracing::info!("J-Quants 日足データ取得元を初期化しました");
                let source: SharedDailyBarSource = request_client.clone();
                (Some(source), Some(request_client), Some(ingest_client))
            }
            _ => {
                tracing::warn!("JQUANTS_API_KEY が未設定のため、日足データ取得元なしで起動します");
                (None, None, None)
            }
        },
        other => {
            return Err(AppError::Config(format!(
                "unknown DATA_PROVIDER value: '{other}' (expected: jquants | ibkr | none)"
            )));
        }
    };

    // t-rader-agent 内部 API client。戦略タスクの投入 / 状態照会を担う。
    let agent_task_client_config =
        AgentTaskClientConfig::from_env().map_err(|e| AppError::Config(e.to_string()))?;
    let strategy_task_reconcile_enabled = matches!(
        &agent_task_client_config,
        AgentTaskClientConfigSource::Configured(_)
    );
    let agent_task_client: SharedAgentTaskClient = match agent_task_client_config {
        AgentTaskClientConfigSource::Configured(config) => {
            let client = HttpAgentTaskClient::new(config).map_err(|e| {
                AppError::Config(format!("failed to initialize agent task client: {e}"))
            })?;
            tracing::info!("agent task client initialized");
            let arc: Arc<dyn AgentTaskClient + Send + Sync> = Arc::new(client);
            arc
        }
        AgentTaskClientConfigSource::Disabled => {
            tracing::warn!(
                "TRADER_AGENT_API_URL=disabled: agent task client を無効化して起動します (dev 用 opt-out)"
            );
            AppState::disabled_agent_task_client()
        }
    };

    // フィード一覧を取り込みごとに読み直し、UI / MCP からの変更を反映する。
    let news_aggregator: SharedNewsAggregator =
        Arc::new(RssNewsAggregator::new(&redis_url).map_err(|err| {
            AppError::Config(format!("failed to initialize RSS news aggregator: {err}"))
        })?);
    let use_cases = backend::services::use_cases::build_use_cases(db.clone());

    let fred_source: Option<SharedIndicatorObservationSource> = match std::env::var("FRED_API_KEY")
    {
        Ok(api_key) if !api_key.is_empty() => {
            let fred_client = FredClient::new(api_key).map_err(|err| {
                AppError::Config(format!("failed to initialize FRED client: {err}"))
            })?;
            Some(Arc::new(fred_client))
        }
        _ => {
            tracing::warn!(
                "FRED_API_KEY が未設定のため、FRED マクロ指標履歴の取り込みを起動しません"
            );
            None
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
        ingest_run_log: Arc::new(PostgresIngestRunLog::new(app_db.clone())),
        fred_source,
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

        let llm_gateway_client =
            LlmGatewayClient::from_env().map(|client| Arc::new(client) as SharedLlmClient);
        let state = AppState {
            db: app_db,
            use_cases,
            daily_bar_source,
            jquants_client,
            agent_task_client,
            agent_webhook_token: Arc::from(agent_webhook_token),
            kata_executor,
            llm_gateway_client,
        };
        Some(create_router(state))
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
            AppError::Config(format!("failed to initialize Graphile Worker: {error}"))
        })?;
        tracing::info!("Graphile Worker initialized");
        Some(worker)
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
            .map_err(|e| AppError::Config(format!("failed to bind to {addr}: {e}")))?;
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

    match (worker, server_run) {
        (Some(worker), Some(mut server_run)) => {
            let mut worker_run = Box::pin(worker.run());

            // 両方を起動した場合は、片方の終了時にもう片方も停止する。
            tokio::select! {
                result = &mut worker_run => {
                    let _ = shutdown_tx.send(true);
                    let worker_result = result
                        .map_err(|error| AppError::Config(format!("Graphile Worker failed: {error}")));
                    let server_result = server_run
                        .await
                        .map_err(|error| AppError::Config(format!("server error: {error}")));
                    worker_result?;
                    server_result
                }
                result = &mut server_run => {
                    let _ = shutdown_tx.send(true);
                    let server_result = result
                        .map_err(|error| AppError::Config(format!("server error: {error}")));
                    let worker_result = worker_run
                        .await
                        .map_err(|error| AppError::Config(format!("Graphile Worker failed: {error}")));
                    server_result?;
                    worker_result
                }
            }
        }
        (Some(worker), None) => worker
            .run()
            .await
            .map_err(|error| AppError::Config(format!("Graphile Worker failed: {error}"))),
        (None, Some(server_run)) => server_run
            .await
            .map_err(|error| AppError::Config(format!("server error: {error}"))),
        // RunMode の追加時に有効化条件が漏れた場合も、無言で終了しないようにする。
        (None, None) => Err(AppError::Config(format!(
            "backend runtime setup is incomplete for run mode {:?}",
            cli.run_mode
        ))),
    }
}

#[cfg(test)]
mod tests {
    use std::io::{self, Write};
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    use rstest::rstest;
    use tokio::time::timeout;

    use super::*;

    #[derive(Clone)]
    struct LogBuffer(Arc<Mutex<Vec<u8>>>);

    struct LogBufferWriter(Arc<Mutex<Vec<u8>>>);

    impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for LogBuffer {
        type Writer = LogBufferWriter;

        fn make_writer(&'a self) -> Self::Writer {
            LogBufferWriter(Arc::clone(&self.0))
        }
    }

    impl Write for LogBufferWriter {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            self.0
                .lock()
                .map_err(|_| io::Error::other("log buffer lock was poisoned"))?
                .extend_from_slice(bytes);
            Ok(bytes.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    fn emitted_logs(filter: tracing_subscriber::EnvFilter) -> String {
        let buffer = Arc::new(Mutex::new(Vec::new()));
        let subscriber = tracing_subscriber::fmt()
            .with_env_filter(filter)
            .without_time()
            .with_ansi(false)
            .with_writer(LogBuffer(Arc::clone(&buffer)))
            .finish();

        tracing::subscriber::with_default(subscriber, || {
            tracing::info!(target: "sqlx::query", "query info");
            tracing::warn!(target: "sqlx::query", "query warning");
            tracing::info!(target: "backend::test", "application info");
        });

        let output = buffer
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        String::from_utf8_lossy(&output).into_owned()
    }

    #[test]
    fn test_default_log_filter_suppresses_sqlx_info_and_keeps_warnings() {
        assert_eq!(
            emitted_logs(default_log_filter()),
            indoc::indoc!(
                "\
                \x20WARN sqlx::query: query warning
                \x20INFO backend::test: application info
                "
            ),
        );
    }

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
    #[tokio::test]
    async fn shutdown_signal_reaches_worker_and_server_waiters() {
        let (sender, receiver) = watch::channel(false);
        let mut worker_waiter = Box::pin(wait_for_shutdown(receiver.clone()));
        let mut server_waiter = Box::pin(wait_for_shutdown(receiver));
        let waiters_are_pending = tokio::select! {
            biased;
            _ = &mut worker_waiter => false,
            _ = &mut server_waiter => false,
            _ = tokio::task::yield_now() => true,
        };
        let signal_sent = sender.send(true).is_ok();
        let waiters_completed = timeout(Duration::from_secs(1), async {
            tokio::join!(worker_waiter, server_waiter);
        })
        .await
        .is_ok();

        assert_eq!(
            (waiters_are_pending, signal_sent, waiters_completed),
            (true, true, true)
        );
    }

    #[rstest]
    #[case::missing(
        None,
        Err("configuration error: REDIS_URL environment variable is not set")
    )]
    #[case::empty(
        Some(String::new()),
        Err("configuration error: REDIS_URL environment variable is not set")
    )]
    #[case::configured(
        Some("redis://localhost/".to_string()),
        Ok("redis://localhost/"),
    )]
    fn test_required_redis_url(
        #[case] value: Option<String>,
        #[case] expected: Result<&str, &str>,
    ) {
        assert_eq!(
            required_redis_url(value).map_err(|error| error.to_string()),
            expected.map(str::to_owned).map_err(str::to_owned),
        );
    }
}
