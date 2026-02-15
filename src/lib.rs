pub mod error;
pub mod handlers;
pub mod models;

use axum::Json;
use axum::Router;
use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::{delete, get, post};
use sqlx::PgPool;

use crate::error::AppError;
use crate::handlers::watchlists;

#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
}

pub fn create_router(state: AppState) -> Router {
    Router::new()
        .route("/api/health", get(health_check))
        .route("/api/watchlists", post(watchlists::create_watchlist))
        .route("/api/watchlists", get(watchlists::list_watchlists))
        .route("/api/watchlists/{id}", delete(watchlists::delete_watchlist))
        .route(
            "/api/watchlists/{id}/items",
            post(watchlists::add_watchlist_item),
        )
        .route(
            "/api/watchlists/{id}/items",
            get(watchlists::list_watchlist_items),
        )
        .route(
            "/api/watchlists/{id}/items/{instrument_id}",
            delete(watchlists::delete_watchlist_item),
        )
        .with_state(state)
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
