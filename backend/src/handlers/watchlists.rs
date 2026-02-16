use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use uuid::Uuid;

use crate::AppState;
use crate::error::AppError;
use crate::models::{AddWatchlistItemRequest, CreateWatchlistRequest, Watchlist, WatchlistItem};

/// ウォッチリストの存在を確認し、存在しない場合は 404 エラーを返す
async fn ensure_watchlist_exists(db: &sqlx::PgPool, watchlist_id: Uuid) -> Result<(), AppError> {
    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM watchlists WHERE id = $1)")
        .bind(watchlist_id)
        .fetch_one(db)
        .await?;

    if !exists {
        return Err(AppError::NotFound(format!(
            "watchlist {watchlist_id} not found"
        )));
    }

    Ok(())
}

pub async fn create_watchlist(
    State(state): State<AppState>,
    Json(payload): Json<CreateWatchlistRequest>,
) -> Result<(StatusCode, Json<Watchlist>), AppError> {
    let name = payload.name.trim().to_string();
    if name.is_empty() {
        return Err(AppError::Validation("name must not be empty".to_string()));
    }

    // sort_order をサブクエリで算出し、INSERT をアトミックに実行する
    let watchlist = sqlx::query_as::<_, Watchlist>(
        "INSERT INTO watchlists (id, name, sort_order) VALUES (gen_random_uuid(), $1, COALESCE((SELECT MAX(sort_order) FROM watchlists), -1) + 1) RETURNING id, name, sort_order, created_at",
    )
    .bind(&name)
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

    ensure_watchlist_exists(&state.db, watchlist_id).await?;

    // 銘柄が存在しない場合は自動作成
    sqlx::query(
        "INSERT INTO instruments (id, name, market) VALUES ($1, $2, 'TSE') ON CONFLICT (id) DO NOTHING",
    )
    .bind(&instrument_id)
    .bind(&name)
    .execute(&state.db)
    .await?;

    // sort_order をサブクエリで算出し、INSERT をアトミックに実行する
    let item = sqlx::query_as::<_, WatchlistItem>(
        "INSERT INTO watchlist_items (watchlist_id, instrument_id, sort_order) VALUES ($1, $2, COALESCE((SELECT MAX(sort_order) FROM watchlist_items WHERE watchlist_id = $1), -1) + 1) RETURNING watchlist_id, instrument_id, sort_order, added_at",
    )
    .bind(watchlist_id)
    .bind(&instrument_id)
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
    ensure_watchlist_exists(&state.db, watchlist_id).await?;

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

#[cfg(test)]
mod tests {
    use serde_json::json;
    use sqlx::PgPool;

    use crate::testing::create_test_server;

    // --- ウォッチリスト作成 ---

    #[sqlx::test]
    async fn create_watchlist_returns_201(pool: PgPool) {
        let server = create_test_server(pool);

        let response = server
            .post("/api/watchlists")
            .json(&json!({ "name": "お気に入り" }))
            .await;

        response.assert_status(axum::http::StatusCode::CREATED);
        let body: serde_json::Value = response.json();
        assert_eq!(body["name"], "お気に入り");
        assert!(body["id"].is_string());
        assert!(body["created_at"].is_string());
    }

    #[sqlx::test]
    async fn create_watchlist_with_empty_name_returns_400(pool: PgPool) {
        let server = create_test_server(pool);

        let response = server
            .post("/api/watchlists")
            .json(&json!({ "name": "" }))
            .await;

        response.assert_status(axum::http::StatusCode::BAD_REQUEST);
        let body: serde_json::Value = response.json();
        assert_eq!(body["error"], "name must not be empty");
    }

    // --- ウォッチリスト一覧 ---

    #[sqlx::test]
    async fn list_watchlists_returns_empty_when_no_data(pool: PgPool) {
        let server = create_test_server(pool);

        let response = server.get("/api/watchlists").await;

        response.assert_status_ok();
        let body: Vec<serde_json::Value> = response.json();
        assert!(body.is_empty());
    }

    #[sqlx::test]
    async fn list_watchlists_contains_created_watchlist(pool: PgPool) {
        let server = create_test_server(pool);

        let create_response = server
            .post("/api/watchlists")
            .json(&json!({ "name": "一覧テスト用" }))
            .await;
        let created: serde_json::Value = create_response.json();
        let created_id = created["id"].as_str().unwrap();

        let response = server.get("/api/watchlists").await;

        response.assert_status_ok();
        let body: Vec<serde_json::Value> = response.json();
        assert_eq!(body.len(), 1);
        assert_eq!(body[0]["id"].as_str(), Some(created_id));
    }

    // --- ウォッチリスト削除 ---

    #[sqlx::test]
    async fn delete_watchlist_returns_204(pool: PgPool) {
        let server = create_test_server(pool);

        let create_response = server
            .post("/api/watchlists")
            .json(&json!({ "name": "削除対象" }))
            .await;
        let id = create_response.json::<serde_json::Value>()["id"]
            .as_str()
            .unwrap()
            .to_string();

        let response = server.delete(&format!("/api/watchlists/{id}")).await;

        response.assert_status(axum::http::StatusCode::NO_CONTENT);
    }

    #[sqlx::test]
    async fn delete_watchlist_not_found_returns_404(pool: PgPool) {
        let server = create_test_server(pool);

        let response = server
            .delete("/api/watchlists/00000000-0000-0000-0000-000000000000")
            .await;

        response.assert_status(axum::http::StatusCode::NOT_FOUND);
    }

    // --- ウォッチリスト項目追加 ---

    #[sqlx::test]
    async fn add_watchlist_item_returns_201(pool: PgPool) {
        let server = create_test_server(pool);

        let create_response = server
            .post("/api/watchlists")
            .json(&json!({ "name": "項目追加テスト" }))
            .await;
        let watchlist_id = create_response.json::<serde_json::Value>()["id"]
            .as_str()
            .unwrap()
            .to_string();

        let response = server
            .post(&format!("/api/watchlists/{watchlist_id}/items"))
            .json(&json!({
                "instrument_id": "7203",
                "name": "トヨタ自動車"
            }))
            .await;

        response.assert_status(axum::http::StatusCode::CREATED);
        let body: serde_json::Value = response.json();
        assert_eq!(body["instrument_id"], "7203");
        assert_eq!(body["sort_order"], 0);
    }

    #[sqlx::test]
    async fn add_watchlist_item_to_nonexistent_watchlist_returns_404(pool: PgPool) {
        let server = create_test_server(pool);

        let response = server
            .post("/api/watchlists/00000000-0000-0000-0000-000000000000/items")
            .json(&json!({
                "instrument_id": "7203",
                "name": "トヨタ自動車"
            }))
            .await;

        response.assert_status(axum::http::StatusCode::NOT_FOUND);
    }

    #[sqlx::test]
    async fn add_duplicate_watchlist_item_returns_400(pool: PgPool) {
        let server = create_test_server(pool);

        let create_response = server
            .post("/api/watchlists")
            .json(&json!({ "name": "重複テスト" }))
            .await;
        let watchlist_id = create_response.json::<serde_json::Value>()["id"]
            .as_str()
            .unwrap()
            .to_string();

        server
            .post(&format!("/api/watchlists/{watchlist_id}/items"))
            .json(&json!({
                "instrument_id": "9984",
                "name": "ソフトバンクグループ"
            }))
            .await;

        let response = server
            .post(&format!("/api/watchlists/{watchlist_id}/items"))
            .json(&json!({
                "instrument_id": "9984",
                "name": "ソフトバンクグループ"
            }))
            .await;

        response.assert_status(axum::http::StatusCode::BAD_REQUEST);
        let body: serde_json::Value = response.json();
        assert!(
            body["error"]
                .as_str()
                .unwrap()
                .contains("already in the watchlist")
        );
    }

    #[sqlx::test]
    async fn add_watchlist_item_with_empty_instrument_id_returns_400(pool: PgPool) {
        let server = create_test_server(pool);

        let create_response = server
            .post("/api/watchlists")
            .json(&json!({ "name": "バリデーションテスト" }))
            .await;
        let watchlist_id = create_response.json::<serde_json::Value>()["id"]
            .as_str()
            .unwrap()
            .to_string();

        let response = server
            .post(&format!("/api/watchlists/{watchlist_id}/items"))
            .json(&json!({
                "instrument_id": "",
                "name": "空のID"
            }))
            .await;

        response.assert_status(axum::http::StatusCode::BAD_REQUEST);
    }

    // --- ウォッチリスト項目一覧 ---

    #[sqlx::test]
    async fn list_watchlist_items_returns_items(pool: PgPool) {
        let server = create_test_server(pool);

        let create_response = server
            .post("/api/watchlists")
            .json(&json!({ "name": "項目一覧テスト" }))
            .await;
        let watchlist_id = create_response.json::<serde_json::Value>()["id"]
            .as_str()
            .unwrap()
            .to_string();

        server
            .post(&format!("/api/watchlists/{watchlist_id}/items"))
            .json(&json!({
                "instrument_id": "8306",
                "name": "三菱UFJフィナンシャル・グループ"
            }))
            .await;
        server
            .post(&format!("/api/watchlists/{watchlist_id}/items"))
            .json(&json!({
                "instrument_id": "8316",
                "name": "三井住友フィナンシャルグループ"
            }))
            .await;

        let response = server
            .get(&format!("/api/watchlists/{watchlist_id}/items"))
            .await;

        response.assert_status_ok();
        let body: Vec<serde_json::Value> = response.json();
        assert_eq!(body.len(), 2);
        assert_eq!(body[0]["instrument_id"], "8306");
        assert_eq!(body[1]["instrument_id"], "8316");
    }

    #[sqlx::test]
    async fn list_items_of_nonexistent_watchlist_returns_404(pool: PgPool) {
        let server = create_test_server(pool);

        let response = server
            .get("/api/watchlists/00000000-0000-0000-0000-000000000000/items")
            .await;

        response.assert_status(axum::http::StatusCode::NOT_FOUND);
    }

    // --- ウォッチリスト項目削除 ---

    #[sqlx::test]
    async fn delete_watchlist_item_returns_204(pool: PgPool) {
        let server = create_test_server(pool);

        let create_response = server
            .post("/api/watchlists")
            .json(&json!({ "name": "項目削除テスト" }))
            .await;
        let watchlist_id = create_response.json::<serde_json::Value>()["id"]
            .as_str()
            .unwrap()
            .to_string();

        server
            .post(&format!("/api/watchlists/{watchlist_id}/items"))
            .json(&json!({
                "instrument_id": "4755",
                "name": "楽天グループ"
            }))
            .await;

        let response = server
            .delete(&format!("/api/watchlists/{watchlist_id}/items/4755"))
            .await;

        response.assert_status(axum::http::StatusCode::NO_CONTENT);

        // 削除後、一覧が空であることを確認
        let list_response = server
            .get(&format!("/api/watchlists/{watchlist_id}/items"))
            .await;
        let body: Vec<serde_json::Value> = list_response.json();
        assert!(body.is_empty());
    }

    #[sqlx::test]
    async fn delete_nonexistent_watchlist_item_returns_404(pool: PgPool) {
        let server = create_test_server(pool);

        let create_response = server
            .post("/api/watchlists")
            .json(&json!({ "name": "存在しない項目削除テスト" }))
            .await;
        let watchlist_id = create_response.json::<serde_json::Value>()["id"]
            .as_str()
            .unwrap()
            .to_string();

        let response = server
            .delete(&format!("/api/watchlists/{watchlist_id}/items/9999"))
            .await;

        response.assert_status(axum::http::StatusCode::NOT_FOUND);
    }
}
