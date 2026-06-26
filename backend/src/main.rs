use std::net::SocketAddr;
use std::sync::Arc;

use backend::AppState;
use backend::cli::Cli;
use backend::create_router;
use backend::data_provider::DataProviderKind;
use backend::data_provider::ibkr::IbkrClient;
use backend::data_provider::jquants::JQuantsClient;
use backend::data_provider::macro_data::stooq::StooqClient;
use backend::data_provider::macro_data::{MacroCache, MacroDataProvider, spawn_poll};
use backend::data_provider::news::NewsAggregator;
use backend::data_provider::news::rss::RssNewsAggregator;
use backend::error::AppError;
use backend::kata_exec::{HttpKataExecutor, KataExecutor, KataExecutorConfig, SharedKataExecutor};
use backend::kubeopencode::{
    HttpKubeopencodeClient, KubeopencodeClient, KubeopencodeConfig, KubeopencodeConfigSource,
    SharedKubeopencodeClient,
};
use clap::Parser;
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

    // 期限切れの MCP session を定期的に削除するバックグラウンドタスクを起動する。
    // 戻り値は意図的に捨てる: 現状の axum::serve は graceful shutdown を取らず、
    // ランタイム終了で task ごと止まる。graceful shutdown を導入する際は cancel token を渡す。
    tracing::info!(
        interval_secs = backend::mcp::store::DEFAULT_GC_INTERVAL.as_secs(),
        "starting mcp session gc task"
    );
    let _gc_task = backend::mcp::store::spawn_gc(
        backend::mcp::PostgresSessionStore::new(db.clone()),
        backend::mcp::store::DEFAULT_GC_INTERVAL,
    );

    let kubeopencode: SharedKubeopencodeClient = match KubeopencodeConfig::from_env()
        .map_err(|e| AppError::Config(e.to_string()))?
    {
        KubeopencodeConfigSource::Configured(config) => {
            let client = HttpKubeopencodeClient::new(config).map_err(|e| {
                AppError::Config(format!("failed to initialize kubeopencode client: {e}"))
            })?;
            tracing::info!("kubeopencode client initialized");
            let arc: Arc<dyn KubeopencodeClient + Send + Sync> = Arc::new(client);
            let _watcher = backend::mcp::watcher::spawn(
                db.clone(),
                arc.clone(),
                backend::mcp::watcher::DEFAULT_INTERVAL,
            );
            arc
        }
        KubeopencodeConfigSource::Disabled => {
            tracing::warn!(
                "KUBEOPENCODE_API_URL=disabled: kubeopencode を無効化して起動します (dev 用 opt-out)"
            );
            AppState::disabled_kubeopencode()
        }
    };

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

    // Stooq から 5min 間隔で macro tick を取得する poll task を起動する
    let macro_cache: Arc<MacroCache> = Arc::new(MacroCache::new());
    let macro_provider: Arc<dyn MacroDataProvider> = Arc::new(StooqClient::new()?);
    let _macro_poll = spawn_poll(
        macro_provider,
        macro_cache.clone(),
        std::time::Duration::from_secs(300),
    );
    tracing::info!("macro data poll task started (Stooq, interval=5min)");

    // 公開 RSS から 1h 間隔でニュースを集約する poll task を起動する
    let news_aggregator: Arc<dyn NewsAggregator> = Arc::new(RssNewsAggregator::new()?);
    let _news_poll = backend::services::news::spawn_poll(
        db.clone(),
        news_aggregator,
        std::time::Duration::from_secs(3600),
    );
    tracing::info!("news aggregation poll task started (public RSS, interval=1h)");

    let state = AppState {
        db,
        data_provider,
        kubeopencode,
        kata_executor,
        macro_cache: Some(macro_cache),
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

    axum::serve(listener, app)
        .await
        .map_err(|e| AppError::Config(format!("server error: {e}")))?;

    Ok(())
}
