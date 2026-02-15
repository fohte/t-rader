use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use uuid::Uuid;

use crate::AppState;
use crate::error::AppError;
use crate::models::{AddWatchlistItemRequest, CreateWatchlistRequest, Watchlist, WatchlistItem};

pub async fn create_watchlist(
    State(state): State<AppState>,
    Json(payload): Json<CreateWatchlistRequest>,
) -> Result<(StatusCode, Json<Watchlist>), AppError> {
    let name = payload.name.trim().to_string();
    if name.is_empty() {
        return Err(AppError::Validation("name must not be empty".to_string()));
    }

    // sort_order は既存の最大値 + 1 にする
    let max_sort: Option<i32> = sqlx::query_scalar("SELECT MAX(sort_order) FROM watchlists")
        .fetch_one(&state.db)
        .await?;
    let sort_order = max_sort.map_or(0, |v| v + 1);

    let watchlist = sqlx::query_as::<_, Watchlist>(
        "INSERT INTO watchlists (id, name, sort_order) VALUES (gen_random_uuid(), $1, $2) RETURNING id, name, sort_order, created_at",
    )
    .bind(&name)
    .bind(sort_order)
    .fetch_one(&state.db)
    .await?;

    Ok((StatusCode::CREATED, Json(watchlist)))
}

pub async fn list_watchlists(
    State(state): State<AppState>,
) -> Result<Json<Vec<Watchlist>>, AppError> {
    let watchlists = sqlx::query_as::<_, Watchlist>(
        "SELECT id, name, sort_order, created_at FROM watchlists ORDER BY sort_order",
    )
    .fetch_all(&state.db)
    .await?;

    Ok(Json(watchlists))
}

pub async fn delete_watchlist(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    let result = sqlx::query("DELETE FROM watchlists WHERE id = $1")
        .bind(id)
        .execute(&state.db)
        .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound(format!("watchlist {id} not found")));
    }

    Ok(StatusCode::NO_CONTENT)
}

pub async fn add_watchlist_item(
    State(state): State<AppState>,
    Path(watchlist_id): Path<Uuid>,
    Json(payload): Json<AddWatchlistItemRequest>,
) -> Result<(StatusCode, Json<WatchlistItem>), AppError> {
    let instrument_id = payload.instrument_id.trim().to_string();
    if instrument_id.is_empty() {
        return Err(AppError::Validation(
            "instrument_id must not be empty".to_string(),
        ));
    }

    let name = payload.name.trim().to_string();
    if name.is_empty() {
        return Err(AppError::Validation("name must not be empty".to_string()));
    }

    // ウォッチリストの存在確認
    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM watchlists WHERE id = $1)")
        .bind(watchlist_id)
        .fetch_one(&state.db)
        .await?;

    if !exists {
        return Err(AppError::NotFound(format!(
            "watchlist {watchlist_id} not found"
        )));
    }

    // 銘柄が存在しない場合は自動作成
    sqlx::query(
        "INSERT INTO instruments (id, name, market) VALUES ($1, $2, 'TSE') ON CONFLICT (id) DO NOTHING",
    )
    .bind(&instrument_id)
    .bind(&name)
    .execute(&state.db)
    .await?;

    // sort_order は既存の最大値 + 1
    let max_sort: Option<i32> =
        sqlx::query_scalar("SELECT MAX(sort_order) FROM watchlist_items WHERE watchlist_id = $1")
            .bind(watchlist_id)
            .fetch_one(&state.db)
            .await?;
    let sort_order = max_sort.map_or(0, |v| v + 1);

    let item = sqlx::query_as::<_, WatchlistItem>(
        "INSERT INTO watchlist_items (watchlist_id, instrument_id, sort_order) VALUES ($1, $2, $3) RETURNING watchlist_id, instrument_id, sort_order, added_at",
    )
    .bind(watchlist_id)
    .bind(&instrument_id)
    .bind(sort_order)
    .fetch_one(&state.db)
    .await
    .map_err(|e| match e {
        sqlx::Error::Database(ref db_err) if db_err.is_unique_violation() => {
            AppError::Validation(format!(
                "instrument {instrument_id} is already in the watchlist"
            ))
        }
        other => AppError::Database(other),
    })?;

    Ok((StatusCode::CREATED, Json(item)))
}

pub async fn list_watchlist_items(
    State(state): State<AppState>,
    Path(watchlist_id): Path<Uuid>,
) -> Result<Json<Vec<WatchlistItem>>, AppError> {
    // ウォッチリストの存在確認
    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM watchlists WHERE id = $1)")
        .bind(watchlist_id)
        .fetch_one(&state.db)
        .await?;

    if !exists {
        return Err(AppError::NotFound(format!(
            "watchlist {watchlist_id} not found"
        )));
    }

    let items = sqlx::query_as::<_, WatchlistItem>(
        "SELECT watchlist_id, instrument_id, sort_order, added_at FROM watchlist_items WHERE watchlist_id = $1 ORDER BY sort_order",
    )
    .bind(watchlist_id)
    .fetch_all(&state.db)
    .await?;

    Ok(Json(items))
}

pub async fn delete_watchlist_item(
    State(state): State<AppState>,
    Path((watchlist_id, instrument_id)): Path<(Uuid, String)>,
) -> Result<StatusCode, AppError> {
    let result =
        sqlx::query("DELETE FROM watchlist_items WHERE watchlist_id = $1 AND instrument_id = $2")
            .bind(watchlist_id)
            .bind(&instrument_id)
            .execute(&state.db)
            .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound(format!(
            "item {instrument_id} not found in watchlist {watchlist_id}"
        )));
    }

    Ok(StatusCode::NO_CONTENT)
}
