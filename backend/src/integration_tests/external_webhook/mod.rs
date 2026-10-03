mod tests_common;

use std::sync::Arc;

use axum::http::StatusCode;
use core_application::agent_task_client::{
    AgentTaskError, FakeAgentTaskClient, SharedAgentTaskClient,
};
use core_application::strategy_task::DEFAULT_PURPOSE;
use gateway_postgres::entities::sea_orm_active_enums::StrategyTaskPhase;
use gateway_postgres::entities::strategy_task;
use sea_orm::{EntityTrait, QueryOrder};
use serde_json::{Value, json};
use uuid::Uuid;

use self::tests_common::{TaskShape, fake_agent_task_client};
use crate::testing::agent_config;
use crate::testing::{
    create_test_server_with_db_and_agent_client, insert_test_hook_trigger, insert_test_strategy,
};

#[backend_test_macros::database_test]
async fn fires_when_event_match_satisfied(db: gateway_postgres::DatabaseHandle) {
    let (db, server) =
        create_test_server_with_db_and_agent_client(db, fake_agent_task_client()).await;
    let strategy_id = insert_test_strategy(&db, "sample-strategy").await;
    agent_config::create(&db, DEFAULT_PURPOSE.to_string())
        .await
        .expect("insert test agent_config");
    let _ = insert_test_hook_trigger(
        &db,
        strategy_id,
        "sample-hook",
        "alert {{payload.symbol}} for {{strategy.name}}",
        Some(json!({"event": {"eq": "fired"}})),
        true,
    )
    .await;

    let res = server
        .post("/api/hooks/sample-hook")
        .json(&json!({"event": "fired", "symbol": "sample-symbol"}))
        .await;
    let mut body: Value = res.json();
    let task_id_str = body["task_id"].as_str().unwrap().to_string();
    let task_id = Uuid::parse_str(&task_id_str).unwrap();
    body["task_id"] = Value::String("<task_id>".to_string());
    let task = strategy_task::Entity::find_by_id(task_id)
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        (res.status_code(), body, TaskShape::from(&task)),
        (
            StatusCode::OK,
            json!({ "fired": true, "task_id": "<task_id>" }),
            TaskShape {
                strategy_id,
                source: "hook".to_string(),
                prompt: "alert sample-symbol for sample-strategy".to_string(),
                phase: StrategyTaskPhase::Running,
            },
        ),
    );
}

#[backend_test_macros::database_test]
async fn skips_when_event_match_not_satisfied(db: gateway_postgres::DatabaseHandle) {
    let (db, server) =
        create_test_server_with_db_and_agent_client(db, fake_agent_task_client()).await;
    let strategy_id = insert_test_strategy(&db, "sample-strategy").await;
    let _ = insert_test_hook_trigger(
        &db,
        strategy_id,
        "sample-hook",
        "sample prompt",
        Some(json!({"event": {"eq": "fired"}})),
        true,
    )
    .await;

    let res = server
        .post("/api/hooks/sample-hook")
        .json(&json!({"event": "ignored"}))
        .await;
    let tasks = strategy_task::Entity::find()
        .order_by_asc(strategy_task::Column::CreatedAt)
        .all(&db)
        .await
        .unwrap();
    assert_eq!(
        (
            res.status_code(),
            res.json::<Value>(),
            tasks.iter().map(TaskShape::from).collect::<Vec<_>>(),
        ),
        (StatusCode::OK, json!({"fired": false}), vec![]),
    );
}

#[backend_test_macros::database_test]
async fn disabled_trigger_is_404(db: gateway_postgres::DatabaseHandle) {
    let (db, server) =
        create_test_server_with_db_and_agent_client(db, fake_agent_task_client()).await;
    let strategy_id = insert_test_strategy(&db, "sample-strategy").await;
    let _ = insert_test_hook_trigger(
        &db,
        strategy_id,
        "disabled-hook",
        "sample prompt",
        None,
        false,
    )
    .await;

    let res = server
        .post("/api/hooks/disabled-hook")
        .json(&json!({}))
        .await;
    assert_eq!(
        (res.status_code(), res.json::<Value>()),
        (
            StatusCode::NOT_FOUND,
            json!({ "error": "hook disabled-hook not found" }),
        ),
    );
}

#[backend_test_macros::database_test]
async fn unknown_slug_is_404(db: gateway_postgres::DatabaseHandle) {
    let (_db, server) =
        create_test_server_with_db_and_agent_client(db, fake_agent_task_client()).await;
    let res = server
        .post("/api/hooks/unknown-hook")
        .json(&json!({}))
        .await;
    assert_eq!(
        (res.status_code(), res.json::<Value>()),
        (
            StatusCode::NOT_FOUND,
            json!({ "error": "hook unknown-hook not found" }),
        ),
    );
}

#[backend_test_macros::database_test]
async fn placeholders_expand_from_payload(db: gateway_postgres::DatabaseHandle) {
    let (db, server) =
        create_test_server_with_db_and_agent_client(db, fake_agent_task_client()).await;
    let strategy_id = insert_test_strategy(&db, "sample-strategy").await;
    agent_config::create(&db, DEFAULT_PURPOSE.to_string())
        .await
        .expect("insert test agent_config");
    let _ = insert_test_hook_trigger(
        &db,
        strategy_id,
        "sample-hook",
        "symbol={{payload.symbol}} price={{payload.price}}",
        None,
        true,
    )
    .await;

    let res = server
        .post("/api/hooks/sample-hook")
        .json(&json!({"symbol": "sample-symbol", "price": 2500}))
        .await;
    let mut body: Value = res.json();
    let task_id = Uuid::parse_str(body["task_id"].as_str().unwrap()).unwrap();
    body["task_id"] = Value::String("<task_id>".to_string());

    let task = strategy_task::Entity::find_by_id(task_id)
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        (res.status_code(), body, TaskShape::from(&task)),
        (
            StatusCode::OK,
            json!({ "fired": true, "task_id": "<task_id>" }),
            TaskShape {
                strategy_id,
                source: "hook".to_string(),
                prompt: "symbol=sample-symbol price=2500".to_string(),
                phase: StrategyTaskPhase::Running,
            },
        ),
    );
}

#[backend_test_macros::database_test]
async fn empty_expanded_prompt_returns_400(db: gateway_postgres::DatabaseHandle) {
    let (db, server) =
        create_test_server_with_db_and_agent_client(db, fake_agent_task_client()).await;
    let strategy_id = insert_test_strategy(&db, "sample-strategy").await;
    let _ = insert_test_hook_trigger(
        &db,
        strategy_id,
        "sample-hook",
        "{{payload.absent}}",
        None,
        true,
    )
    .await;

    let res = server.post("/api/hooks/sample-hook").json(&json!({})).await;
    let tasks = strategy_task::Entity::find()
        .order_by_asc(strategy_task::Column::CreatedAt)
        .all(&db)
        .await
        .unwrap();
    assert_eq!(
        (
            res.status_code(),
            res.json::<Value>(),
            tasks.iter().map(TaskShape::from).collect::<Vec<_>>(),
        ),
        (
            StatusCode::BAD_REQUEST,
            json!({ "error": "prompt must not be empty" }),
            vec![],
        ),
    );
}

#[backend_test_macros::database_test]
async fn agent_not_configured_returns_503(db: gateway_postgres::DatabaseHandle) {
    let fake = Arc::new(FakeAgentTaskClient::new());
    fake.set_submit_error(AgentTaskError::NotConfigured).await;
    let agent_client: SharedAgentTaskClient = fake;
    let (db, server) = create_test_server_with_db_and_agent_client(db, agent_client).await;
    let strategy_id = insert_test_strategy(&db, "sample-strategy").await;
    agent_config::create(&db, DEFAULT_PURPOSE.to_string())
        .await
        .expect("insert test agent_config");
    let _ = insert_test_hook_trigger(&db, strategy_id, "sample-hook", "sample prompt", None, true)
        .await;

    let res = server.post("/api/hooks/sample-hook").json(&json!({})).await;
    assert_eq!(
        (res.status_code(), res.json::<Value>()),
        (
            StatusCode::SERVICE_UNAVAILABLE,
            json!({ "error": "agent task client is not configured" }),
        ),
    );
}

#[backend_test_macros::database_test]
async fn missing_default_agent_config_returns_503(db: gateway_postgres::DatabaseHandle) {
    let (db, server) =
        create_test_server_with_db_and_agent_client(db, fake_agent_task_client()).await;
    let strategy_id = insert_test_strategy(&db, "sample-strategy").await;
    let _ = insert_test_hook_trigger(&db, strategy_id, "sample-hook", "sample prompt", None, true)
        .await;

    let res = server.post("/api/hooks/sample-hook").json(&json!({})).await;
    assert_eq!(
        (
            res.status_code(),
            res.json::<Value>(),
            strategy_task::Entity::find()
                .order_by_asc(strategy_task::Column::CreatedAt)
                .all(&db)
                .await
                .unwrap()
                .iter()
                .map(TaskShape::from)
                .collect::<Vec<_>>(),
        ),
        (
            StatusCode::SERVICE_UNAVAILABLE,
            json!({ "error": "agent_config for purpose 'default' not found" }),
            vec![],
        ),
    );
}
