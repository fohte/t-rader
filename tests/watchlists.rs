#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    reason = "テストコードでは expect/unwrap を許可する"
)]

use axum_test::TestServer;
use backend::{AppState, create_router};
use serde_json::json;
use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;

/// テスト用サーバーを作成する。
/// 各テストは独立したデータを使い、他テストのデータに依存しない。
async fn setup() -> (TestServer, PgPool) {
    let database_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| {
        "postgres://t_rader:t_rader@localhost:5432/t_rader_development".to_string()
    });

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
        .expect("failed to connect to database");

    sqlx::migrate!()
        .run(&pool)
        .await
        .expect("failed to run migrations");

    let state = AppState { db: pool.clone() };
    let router = create_router(state);
    let server = TestServer::new(router).expect("failed to create test server");
    (server, pool)
}

/// テストで作成したウォッチリストを削除する
async fn teardown_watchlist(pool: &PgPool, watchlist_id: &str) {
    // ON DELETE CASCADE により watchlist_items も削除される
    sqlx::query("DELETE FROM watchlists WHERE id = $1::uuid")
        .bind(watchlist_id)
        .execute(pool)
        .await
        .expect("failed to delete watchlist");
}

// --- ウォッチリスト作成 ---

#[tokio::test]
async fn create_watchlist_returns_201() {
    let (server, pool) = setup().await;

    let response = server
        .post("/api/watchlists")
        .json(&json!({ "name": "お気に入り" }))
        .await;

    response.assert_status(axum::http::StatusCode::CREATED);
    let body: serde_json::Value = response.json();
    assert_eq!(body["name"], "お気に入り");
    assert!(body["id"].is_string());
    assert!(body["created_at"].is_string());

    teardown_watchlist(&pool, body["id"].as_str().unwrap()).await;
}

#[tokio::test]
async fn create_watchlist_with_empty_name_returns_400() {
    let (server, _pool) = setup().await;

    let response = server
        .post("/api/watchlists")
        .json(&json!({ "name": "" }))
        .await;

    response.assert_status(axum::http::StatusCode::BAD_REQUEST);
    let body: serde_json::Value = response.json();
    assert_eq!(body["error"], "name must not be empty");
}

// --- ウォッチリスト一覧 ---

#[tokio::test]
async fn list_watchlists_contains_created_watchlist() {
    let (server, pool) = setup().await;

    let create_response = server
        .post("/api/watchlists")
        .json(&json!({ "name": "一覧テスト用" }))
        .await;
    let created: serde_json::Value = create_response.json();
    let created_id = created["id"].as_str().unwrap();

    let response = server.get("/api/watchlists").await;

    response.assert_status_ok();
    let body: Vec<serde_json::Value> = response.json();
    assert!(body.iter().any(|w| w["id"].as_str() == Some(created_id)));

    teardown_watchlist(&pool, created_id).await;
}

// --- ウォッチリスト削除 ---

#[tokio::test]
async fn delete_watchlist_returns_204() {
    let (server, _pool) = setup().await;

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

#[tokio::test]
async fn delete_watchlist_not_found_returns_404() {
    let (server, _pool) = setup().await;

    let response = server
        .delete("/api/watchlists/00000000-0000-0000-0000-000000000000")
        .await;

    response.assert_status(axum::http::StatusCode::NOT_FOUND);
}

// --- ウォッチリスト項目追加 ---

#[tokio::test]
async fn add_watchlist_item_returns_201() {
    let (server, pool) = setup().await;

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

    teardown_watchlist(&pool, &watchlist_id).await;
}

#[tokio::test]
async fn add_watchlist_item_to_nonexistent_watchlist_returns_404() {
    let (server, _pool) = setup().await;

    let response = server
        .post("/api/watchlists/00000000-0000-0000-0000-000000000000/items")
        .json(&json!({
            "instrument_id": "7203",
            "name": "トヨタ自動車"
        }))
        .await;

    response.assert_status(axum::http::StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn add_duplicate_watchlist_item_returns_400() {
    let (server, pool) = setup().await;

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

    teardown_watchlist(&pool, &watchlist_id).await;
}

#[tokio::test]
async fn add_watchlist_item_with_empty_instrument_id_returns_400() {
    let (server, pool) = setup().await;

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

    teardown_watchlist(&pool, &watchlist_id).await;
}

// --- ウォッチリスト項目一覧 ---

#[tokio::test]
async fn list_watchlist_items_returns_items() {
    let (server, pool) = setup().await;

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

    teardown_watchlist(&pool, &watchlist_id).await;
}

#[tokio::test]
async fn list_items_of_nonexistent_watchlist_returns_404() {
    let (server, _pool) = setup().await;

    let response = server
        .get("/api/watchlists/00000000-0000-0000-0000-000000000000/items")
        .await;

    response.assert_status(axum::http::StatusCode::NOT_FOUND);
}

// --- ウォッチリスト項目削除 ---

#[tokio::test]
async fn delete_watchlist_item_returns_204() {
    let (server, pool) = setup().await;

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

    teardown_watchlist(&pool, &watchlist_id).await;
}

#[tokio::test]
async fn delete_nonexistent_watchlist_item_returns_404() {
    let (server, pool) = setup().await;

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

    teardown_watchlist(&pool, &watchlist_id).await;
}
