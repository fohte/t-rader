use std::net::SocketAddr;

use backend::AppState;
use backend::create_router;
use backend::error::AppError;
use sqlx::postgres::PgPoolOptions;

#[tokio::main]
async fn main() -> Result<(), AppError> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let database_url = std::env::var("DATABASE_URL").map_err(|_| {
        AppError::Config("DATABASE_URL environment variable is not set".to_string())
    })?;

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await?;

    tracing::info!("running database migrations");
    sqlx::migrate!().run(&pool).await?;
    tracing::info!("database migrations completed");

    let state = AppState { db: pool };

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
