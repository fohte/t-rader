use std::net::SocketAddr;

use backend::AppState;
use backend::create_router;
use backend::error::AppError;
use migration::{Migrator, MigratorTrait};
use sea_orm::{ConnectOptions, Database};

#[tokio::main]
async fn main() -> Result<(), AppError> {
    // --dump-openapi: OpenAPI スペックを JSON で標準出力に出力して終了する
    if std::env::args().any(|arg| arg == "--dump-openapi") {
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

    tracing::info!("running database migrations");
    Migrator::up(&db, None).await?;
    tracing::info!("database migrations completed");

    let state = AppState { db };

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
