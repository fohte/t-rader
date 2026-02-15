#[cfg(test)]
mod data_provider;
mod error;
#[cfg(test)]
mod models;

use std::net::SocketAddr;

use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;

use crate::error::AppError;

#[derive(Clone)]
struct AppState {
    db: PgPool,
}

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

    let app = Router::new()
        .route("/api/health", get(health_check))
        .with_state(state);

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

async fn health_check(
    State(state): State<AppState>,
) -> Result<(StatusCode, Json<serde_json::Value>), AppError> {
    // DB 接続の正常性を確認
    sqlx::query("SELECT 1").execute(&state.db).await?;

    Ok((
        StatusCode::OK,
        Json(serde_json::json!({
            "status": "ok",
        })),
    ))
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    #[rstest]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
