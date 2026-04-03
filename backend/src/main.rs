use std::net::SocketAddr;
use std::sync::Arc;

use backend::AppState;
use backend::cli::Cli;
use backend::create_router;
use backend::data_provider::DataProviderKind;
use backend::data_provider::jquants::JQuantsClient;
use backend::error::AppError;
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

    // J-Quants API キーが設定されている場合のみ DataProvider を初期化する
    let data_provider = match std::env::var("JQUANTS_API_KEY") {
        Ok(api_key) if !api_key.is_empty() => {
            let client = JQuantsClient::new(api_key)?;
            tracing::info!("J-Quants DataProvider を初期化しました");
            Some(Arc::new(DataProviderKind::JQuants(client)))
        }
        _ => {
            tracing::warn!("JQUANTS_API_KEY が未設定のため、DataProvider なしで起動します");
            None
        }
    };

    let state = AppState { db, data_provider };

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
