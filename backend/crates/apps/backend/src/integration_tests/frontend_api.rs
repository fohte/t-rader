use gateway_postgres::entities::sea_orm_active_enums::StrategyTaskPhase;
use gateway_postgres::entities::strategy_task;
use serde_json::{Value, json};
use uuid::Uuid;

fn assert_response_eq(
    response: &axum_test::TestResponse,
    expected_status: axum::http::StatusCode,
    expected_body: Option<serde_json::Value>,
) {
    let actual_body = if response.as_bytes().is_empty() {
        None
    } else {
        Some(response.json::<serde_json::Value>())
    };

    assert_eq!(
        (response.status_code(), actual_body),
        (expected_status, expected_body),
    );
}

fn normalize_strategy(mut value: Value) -> Value {
    for key in ["created_at", "updated_at"] {
        if let Some(field) = value.get_mut(key) {
            *field = Value::String(format!("<{key}>"));
        }
    }
    value
}

async fn create_strategy(server: &axum_test::TestServer, name: &str) -> String {
    create_strategy_with_description(server, name, None).await
}

async fn create_strategy_with_description(
    server: &axum_test::TestServer,
    name: &str,
    description: Option<&str>,
) -> String {
    let request_body = match description {
        Some(description) => json!({ "name": name, "description": description }),
        None => json!({ "name": name }),
    };
    let response = server.post("/api/strategies").json(&request_body).await;
    let mut body: Value = response.json();
    let id = body["id"].as_str().expect("id").to_string();
    body = normalize_strategy(body);
    assert_eq!(
        (response.status_code(), body),
        (
            axum::http::StatusCode::CREATED,
            json!({
                "id": id,
                "name": name,
                "description": description,
                "sort_order": 0,
                "created_at": "<created_at>",
                "updated_at": "<updated_at>",
            }),
        ),
    );
    id
}

async fn create_agent_config(server: &axum_test::TestServer, purpose: &str) {
    let response = server
        .post("/api/agent-configs")
        .json(&json!({ "purpose": purpose }))
        .await;
    let mut body: Value = response.json();
    for key in ["id", "created_at", "updated_at"] {
        if let Some(value) = body.get_mut(key) {
            *value = json!(format!("<{key}>"));
        }
    }
    assert_eq!(
        (response.status_code(), body),
        (
            axum::http::StatusCode::CREATED,
            json!({
                "id": "<id>",
                "purpose": purpose,
                "agents_md": "",
                "skills": {},
                "agent_graph": "",
                "created_at": "<created_at>",
                "updated_at": "<updated_at>",
            }),
        ),
    );
}

fn normalize_note_response(mut value: Value) -> Value {
    for key in ["id", "version_id", "created_at", "updated_at"] {
        if let Some(field) = value.as_object_mut().and_then(|object| object.get_mut(key)) {
            *field = json!(format!("<{key}>"));
        }
    }
    value
}

fn normalize_annotation_response(mut value: Value) -> Value {
    fn normalize_row(annotation: &mut Value) {
        for key in ["id", "created_at", "updated_at"] {
            if let Some(field) = annotation
                .as_object_mut()
                .and_then(|object| object.get_mut(key))
            {
                *field = json!(format!("<{key}>"));
            }
        }
    }
    if let Some(rows) = value.as_array_mut() {
        for row in rows {
            normalize_row(row);
        }
    } else {
        normalize_row(&mut value);
    }
    value
}

fn normalize_timestamps(value: &mut Value) {
    if let Some(object) = value.as_object_mut() {
        for key in ["created_at", "updated_at", "as_of"] {
            if let Some(field) = object.get_mut(key)
                && !field.is_null()
            {
                *field = json!(format!("<{key}>"));
            }
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
struct TaskShape {
    strategy_id: Uuid,
    source: String,
    prompt: String,
    phase: StrategyTaskPhase,
}

impl TaskShape {
    fn from(row: &strategy_task::Model) -> Self {
        Self {
            strategy_id: row.strategy_id,
            source: row.source.clone(),
            prompt: row.prompt.clone(),
            phase: row.phase.clone(),
        }
    }
}

mod agent_config;
mod agent_options;
mod annotations;
mod bars;
mod comments;
mod config;
mod custom_indicators;
mod group_axes;
mod history;
mod ingest_status;
mod note_kinds;
mod note_predictions;
mod note_versions;
mod notes;
mod refs;
mod risk_policy;
mod rss_feeds;
mod strategies;
mod strategy_investable_amount;
mod strategy_tasks;
mod tasks;
mod trade_notes;
mod trades;
mod triggers;
