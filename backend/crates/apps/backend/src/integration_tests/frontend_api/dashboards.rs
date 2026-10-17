use axum::http::StatusCode;
use indoc::indoc;
use serde_json::{Value, json};

use crate::testing::create_test_server;

const CONFIG: &str = indoc! {"
    title: sample-dashboard
    widgets: []
"};

const UPDATED_CONFIG: &str = indoc! {"
    title: updated-dashboard
    widgets: []
"};

fn create_request(key: &str, config: &str) -> Value {
    json!({ "key": key, "config": config })
}

fn normalize_dashboard(mut dashboard: Value) -> Value {
    for key in ["created_at", "updated_at"] {
        dashboard[key] = json!(format!("<{key}>"));
    }
    dashboard
}

#[backend_test_macros::database_test]
async fn create_returns_a_dashboard_that_can_be_read_and_listed(
    db: gateway_postgres::DatabaseHandle,
) {
    let server = create_test_server(db).await;
    let created = server
        .post("/api/dashboards")
        .json(&create_request("sample-dashboard", CONFIG))
        .await;
    let read = server.get("/api/dashboards/sample-dashboard").await;
    let list = server.get("/api/dashboards").await;
    let list_body: Value = list.json();
    let normalized_list = list_body.as_array().map(|rows| {
        rows.iter()
            .cloned()
            .map(normalize_dashboard)
            .collect::<Vec<_>>()
    });

    let expected = json!({
        "key": "sample-dashboard",
        "config": CONFIG,
        "created_at": "<created_at>",
        "updated_at": "<updated_at>",
    });
    assert_eq!(
        (
            created.status_code(),
            normalize_dashboard(created.json()),
            read.status_code(),
            normalize_dashboard(read.json()),
            list.status_code(),
            normalized_list,
        ),
        (
            StatusCode::CREATED,
            expected.clone(),
            StatusCode::OK,
            expected.clone(),
            StatusCode::OK,
            Some(vec![expected]),
        ),
    );
}

#[backend_test_macros::database_test]
async fn update_replaces_config_and_delete_removes_dashboard(db: gateway_postgres::DatabaseHandle) {
    let server = create_test_server(db).await;
    server
        .post("/api/dashboards")
        .json(&create_request("sample-dashboard", CONFIG))
        .await;
    let updated = server
        .put("/api/dashboards/sample-dashboard")
        .json(&json!({ "config": UPDATED_CONFIG }))
        .await;
    let current = server.get("/api/dashboards/sample-dashboard").await;
    let deleted = server.delete("/api/dashboards/sample-dashboard").await;
    let missing = server.get("/api/dashboards/sample-dashboard").await;

    assert_eq!(
        (
            updated.status_code(),
            normalize_dashboard(updated.json()),
            current.status_code(),
            normalize_dashboard(current.json()),
            deleted.status_code(),
            missing.status_code(),
            missing.json::<Value>(),
        ),
        (
            StatusCode::OK,
            json!({
                "key": "sample-dashboard",
                "config": UPDATED_CONFIG,
                "created_at": "<created_at>",
                "updated_at": "<updated_at>",
            }),
            StatusCode::OK,
            json!({
                "key": "sample-dashboard",
                "config": UPDATED_CONFIG,
                "created_at": "<created_at>",
                "updated_at": "<updated_at>",
            }),
            StatusCode::NO_CONTENT,
            StatusCode::NOT_FOUND,
            json!({ "error": "dashboard 'sample-dashboard' not found" }),
        ),
    );
}

#[backend_test_macros::database_test]
async fn invalid_config_is_rejected_before_storage(db: gateway_postgres::DatabaseHandle) {
    let server = create_test_server(db).await;
    let invalid = server
        .post("/api/dashboards")
        .json(&create_request(
            "sample-dashboard",
            indoc! {"
                title: sample-dashboard
                widgets:
                  - title: sample-widget
                    width: half
                    queries: {}
                    viz:
                      type: toplist
                      value: missing
                      order: desc
                      limit: 1
            "},
        ))
        .await;
    let list = server.get("/api/dashboards").await;

    assert_eq!(
        (
            invalid.status_code(),
            invalid.json::<Value>(),
            list.status_code(),
            list.json::<Value>(),
        ),
        (
            StatusCode::BAD_REQUEST,
            json!({
                "error": "widgets[0].queries: at least one query is required; widgets[0].viz.value: unknown query or formula name: `missing`"
            }),
            StatusCode::OK,
            json!([]),
        ),
    );
}

#[backend_test_macros::database_test]
async fn duplicate_dashboard_key_returns_conflict(db: gateway_postgres::DatabaseHandle) {
    let server = create_test_server(db).await;
    let created = server
        .post("/api/dashboards")
        .json(&create_request("sample-dashboard", CONFIG))
        .await;
    let duplicate = server
        .post("/api/dashboards")
        .json(&create_request("sample-dashboard", CONFIG))
        .await;

    assert_eq!(
        (
            created.status_code(),
            duplicate.status_code(),
            duplicate.json::<Value>(),
        ),
        (
            StatusCode::CREATED,
            StatusCode::CONFLICT,
            json!({ "error": "dashboard 'sample-dashboard' already exists" }),
        ),
    );
}

#[backend_test_macros::database_test]
async fn invalid_update_preserves_the_saved_config(db: gateway_postgres::DatabaseHandle) {
    let server = create_test_server(db).await;
    server
        .post("/api/dashboards")
        .json(&create_request("sample-dashboard", CONFIG))
        .await;
    let invalid_update = server
        .put("/api/dashboards/sample-dashboard")
        .json(&json!({
            "config": indoc! {"
                title: sample-dashboard
                widgets:
                  - title: sample-widget
                    width: half
                    queries: {}
                    viz:
                      type: toplist
                      value: missing
                      order: desc
                      limit: 1
            "}
        }))
        .await;
    let current = server.get("/api/dashboards/sample-dashboard").await;

    assert_eq!(
        (
            invalid_update.status_code(),
            invalid_update.json::<Value>(),
            current.status_code(),
            normalize_dashboard(current.json()),
        ),
        (
            StatusCode::BAD_REQUEST,
            json!({
                "error": "widgets[0].queries: at least one query is required; widgets[0].viz.value: unknown query or formula name: `missing`"
            }),
            StatusCode::OK,
            json!({
                "key": "sample-dashboard",
                "config": CONFIG,
                "created_at": "<created_at>",
                "updated_at": "<updated_at>",
            }),
        ),
    );
}
