use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use core_application::strategy_scope::{
    StrategyScope, StrategyScopeError, StrategyScopeSourceError,
};
use core_application::strategy_task::{
    GetTaskError, ListTasksError, StrategyTaskRepositoryError, SubmitTaskError, TaskSource,
};
use core_application::unit_of_work::UnitOfWorkError;
use uuid::Uuid;

use crate::AppState;
use crate::agent_client::AgentTaskError;
use crate::error::{AppError, ErrorResponse};
use crate::extractors::{JsonBody, JsonPath};
use crate::models::{
    StrategyChatRequest, StrategyChatResponse, StrategyTaskStatusResponse, StrategyTaskSummary,
};
pub(crate) fn map_submit_error(err: SubmitTaskError) -> AppError {
    match err {
        SubmitTaskError::EmptyPrompt => AppError::Validation("prompt must not be empty".into()),
        SubmitTaskError::StrategyNotFound(id) => {
            AppError::NotFound(format!("strategy {id} not found"))
        }
        SubmitTaskError::PurposeNotFound(purpose) => {
            AppError::ServiceUnavailable(format!("agent_config for purpose '{purpose}' not found"))
        }
        SubmitTaskError::Repository(StrategyTaskRepositoryError::Database(error))
        | SubmitTaskError::UnitOfWork(UnitOfWorkError::Begin(error))
        | SubmitTaskError::UnitOfWork(UnitOfWorkError::Commit(error)) => error.into(),
        SubmitTaskError::Repository(StrategyTaskRepositoryError::InvalidTransaction)
        | SubmitTaskError::UnitOfWork(UnitOfWorkError::InvalidTransaction) => AppError::Database(
            sea_orm::DbErr::Custom("invalid strategy task transaction".into()),
        ),
        SubmitTaskError::AgentTask(AgentTaskError::NotConfigured) => {
            AppError::ServiceUnavailable("agent task client is not configured".into())
        }
        SubmitTaskError::AgentTask(agent_err) => {
            AppError::Config(format!("agent task error: {agent_err}"))
        }
    }
}

pub(crate) fn map_list_task_error(error: ListTasksError) -> AppError {
    match error {
        ListTasksError::Repository(StrategyTaskRepositoryError::Database(error)) => error.into(),
        ListTasksError::Repository(StrategyTaskRepositoryError::InvalidTransaction) => {
            AppError::Database(sea_orm::DbErr::Custom(
                "invalid strategy task transaction".into(),
            ))
        }
    }
}

fn map_get_task_error(error: GetTaskError) -> AppError {
    match error {
        GetTaskError::NotFound(id) => AppError::NotFound(format!("strategy task {id} not found")),
        GetTaskError::StrategyMismatch { task_id, .. } => {
            AppError::NotFound(format!("strategy task {task_id} not found"))
        }
        GetTaskError::Repository(StrategyTaskRepositoryError::Database(error)) => error.into(),
        GetTaskError::Repository(StrategyTaskRepositoryError::InvalidTransaction) => {
            AppError::Database(sea_orm::DbErr::Custom(
                "invalid strategy task transaction".into(),
            ))
        }
    }
}

async fn verify_strategy_scope(
    state: &AppState,
    strategy_id: Uuid,
    not_found_message: String,
) -> Result<StrategyScope, AppError> {
    crate::services::strategies::strategy_scope(&state.db, strategy_id)
        .await
        .map_err(|error| match error {
            StrategyScopeError::NotFound(_) => AppError::NotFound(not_found_message),
            StrategyScopeError::Source(StrategyScopeSourceError::QueryFailed(message)) => {
                AppError::Database(sea_orm::DbErr::Custom(message))
            }
        })
}

/// フローティングチャットから戦略 Agent にタスクを投入する
#[utoipa::path(
    post,
    path = "/api/strategies/{id}/chat",
    tag = "strategies",
    params(("id" = Uuid, Path, description = "戦略 ID")),
    request_body = StrategyChatRequest,
    responses(
        (status = 202, body = StrategyChatResponse),
        (status = 400, description = "prompt が空 (空白のみを含む)、または指定された purpose の agent_config が存在しない", body = ErrorResponse),
        (status = 404, description = "戦略が存在しない", body = ErrorResponse),
        (status = 415, description = "Content-Type ヘッダが application/json ではない", body = ErrorResponse),
        (status = 422, description = "リクエストボディのパースに失敗", body = ErrorResponse),
        (status = 500, body = ErrorResponse),
        (status = 503, description = "agent task client が未設定、または既定の agent_config が見つからない", body = ErrorResponse),
    )
)]
pub async fn submit_strategy_chat(
    State(state): State<AppState>,
    JsonPath(id): JsonPath<Uuid>,
    JsonBody(payload): JsonBody<StrategyChatRequest>,
) -> Result<(StatusCode, Json<StrategyChatResponse>), AppError> {
    let requested_purpose = payload.purpose;
    let submitted = state
        .use_cases
        .strategy_tasks
        .submit_task(
            state.agent_task_client.as_ref(),
            id,
            &payload.prompt,
            TaskSource::Frontend,
            requested_purpose.clone(),
        )
        .await
        .map_err(|error| match error {
            err @ SubmitTaskError::PurposeNotFound(_) if requested_purpose.is_some() => {
                AppError::Validation(err.to_string())
            }
            error => map_submit_error(error),
        })?;
    Ok((
        StatusCode::ACCEPTED,
        Json(StrategyChatResponse {
            task_id: submitted.task_id,
            a2a_task_id: submitted.a2a_task_id,
        }),
    ))
}

/// 投入済み戦略タスクの phase / error_summary を取得する
#[utoipa::path(
    get,
    path = "/api/strategies/{id}/tasks/{task_id}",
    tag = "strategies",
    params(
        ("id" = Uuid, Path, description = "戦略 ID"),
        ("task_id" = Uuid, Path, description = "戦略タスク ID"),
    ),
    responses(
        (status = 200, body = StrategyTaskStatusResponse),
        (status = 400, description = "パスパラメータが不正", body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn get_strategy_task(
    State(state): State<AppState>,
    JsonPath((strategy_id, task_id)): JsonPath<(Uuid, Uuid)>,
) -> Result<Json<StrategyTaskStatusResponse>, AppError> {
    let scope = verify_strategy_scope(
        &state,
        strategy_id,
        format!("strategy task {task_id} not found"),
    )
    .await?;
    let view = state
        .use_cases
        .strategy_tasks
        .get_for_strategy(scope, task_id)
        .await
        .map_err(map_get_task_error)?;
    Ok(Json(StrategyTaskStatusResponse {
        task_id: view.task_id,
        strategy_id: view.strategy_id,
        a2a_task_id: view.a2a_task_id,
        source: view.source,
        prompt: view.prompt,
        phase: view.phase.as_str().to_string(),
        error_summary: view.error_summary,
        result_text: view.result_text,
        created_at: view.created_at,
        updated_at: view.updated_at,
        steps: view.steps,
        purpose: view.purpose,
        as_of: view.as_of,
    }))
}

/// 戦略の過去タスクを新しい順に一覧取得する
#[utoipa::path(
    get,
    path = "/api/strategies/{id}/tasks",
    tag = "strategies",
    params(("id" = Uuid, Path, description = "戦略 ID")),
    responses(
        (status = 200, body = Vec<StrategyTaskSummary>),
        (status = 400, description = "パスパラメータが不正", body = ErrorResponse),
        (status = 404, description = "戦略が存在しない", body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn list_strategy_tasks(
    State(state): State<AppState>,
    JsonPath(strategy_id): JsonPath<Uuid>,
) -> Result<Json<Vec<StrategyTaskSummary>>, AppError> {
    let scope = verify_strategy_scope(
        &state,
        strategy_id,
        format!("strategy {strategy_id} not found"),
    )
    .await?;
    let views = state
        .use_cases
        .strategy_tasks
        .list_for_strategy(scope, None)
        .await
        .map_err(map_list_task_error)?;
    Ok(Json(
        views.into_iter().map(StrategyTaskSummary::from).collect(),
    ))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use sea_orm::EntityTrait;
    use serde_json::json;
    use uuid::Uuid;

    use crate::agent_client::{AgentTaskError, FakeAgentTaskClient, SharedAgentTaskClient};
    use crate::services::agent_config;
    use crate::services::strategy_tasks::DEFAULT_PURPOSE;
    use crate::testing::{
        create_test_server, create_test_server_with_db,
        create_test_server_with_db_and_agent_client, insert_test_strategy,
        insert_test_strategy_task,
    };
    use gateway_postgres::entities::strategy_task;

    /// JSON body から動的フィールド (created_at/updated_at/as_of) を除去し、
    /// 単一の assert_eq! で残りのフィールドを比較できるようにする。
    fn strip_timestamps(v: &mut serde_json::Value) {
        if let Some(obj) = v.as_object_mut() {
            obj.remove("created_at");
            obj.remove("updated_at");
            obj.remove("as_of");
        }
    }

    #[backend_test_macros::database_test]
    async fn submit_chat_creates_task_row_and_submits_to_agent(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let fake = Arc::new(FakeAgentTaskClient::new());
        fake.set_next_task_id("agent-task-1").await;
        let agent_client: SharedAgentTaskClient = fake.clone();
        let (db, server) = create_test_server_with_db_and_agent_client(db, agent_client).await;
        let strategy_id = insert_test_strategy(&db, "long").await;
        agent_config::create(&db, DEFAULT_PURPOSE.to_string())
            .await
            .expect("insert test agent_config");

        let res = server
            .post(&format!("/api/strategies/{strategy_id}/chat"))
            .json(&json!({ "prompt": " inspect 7203 " }))
            .await;
        res.assert_status(axum::http::StatusCode::ACCEPTED);

        let mut body: serde_json::Value = res.json();
        let task_id = Uuid::parse_str(body["task_id"].as_str().expect("task_id")).expect("uuid");
        body["task_id"] = json!("<uuid>");
        assert_eq!(
            body,
            json!({
                "task_id": "<uuid>",
                "a2a_task_id": "agent-task-1",
            }),
        );

        let row = strategy_task::Entity::find_by_id(task_id)
            .one(&db)
            .await
            .unwrap()
            .expect("row");
        let row_summary = (
            row.task_id,
            row.strategy_id,
            row.a2a_task_id,
            row.source,
            row.prompt,
            row.phase,
            row.error_summary,
        );
        assert_eq!(
            row_summary,
            (
                task_id,
                strategy_id,
                Some("agent-task-1".to_string()),
                "frontend".to_string(),
                "inspect 7203".to_string(),
                gateway_postgres::entities::sea_orm_active_enums::StrategyTaskPhase::Running,
                None,
            ),
        );

        let submitted: Vec<(Uuid, String)> = fake
            .submitted
            .lock()
            .await
            .iter()
            .map(|s| (s.strategy_id, s.prompt.clone()))
            .collect();
        assert_eq!(submitted, vec![(strategy_id, "inspect 7203".to_string())]);
    }

    #[backend_test_macros::database_test]
    async fn submit_chat_uses_requested_purpose(db: gateway_postgres::DatabaseHandle) {
        let fake = Arc::new(FakeAgentTaskClient::new());
        fake.set_next_task_id("purpose-task").await;
        let agent_client: SharedAgentTaskClient = fake.clone();
        let (db, server) = create_test_server_with_db_and_agent_client(db, agent_client).await;
        let strategy_id = insert_test_strategy(&db, "example-strategy").await;
        agent_config::create(&db, "example-purpose".to_string())
            .await
            .expect("insert test agent_config");

        let res = server
            .post(&format!("/api/strategies/{strategy_id}/chat"))
            .json(&json!({ "prompt": "inspect the example", "purpose": "example-purpose" }))
            .await;
        let status = res.status_code();
        let mut body: serde_json::Value = res.json();
        let task_id = Uuid::parse_str(body["task_id"].as_str().expect("task_id")).expect("uuid");
        body["task_id"] = json!("<uuid>");

        let row = strategy_task::Entity::find_by_id(task_id)
            .one(&db)
            .await
            .unwrap()
            .expect("row");
        let submitted_purpose = fake
            .submitted
            .lock()
            .await
            .first()
            .map(|submitted| submitted.purpose.clone());

        assert_eq!(
            (status, body, row.purpose, submitted_purpose),
            (
                axum::http::StatusCode::ACCEPTED,
                json!({
                    "task_id": "<uuid>",
                    "a2a_task_id": "purpose-task",
                }),
                Some("example-purpose".to_string()),
                Some(Some("example-purpose".to_string())),
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn submit_chat_unknown_strategy_returns_404(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        let res = server
            .post("/api/strategies/00000000-0000-0000-0000-000000000000/chat")
            .json(&json!({ "prompt": "x" }))
            .await;
        res.assert_status(axum::http::StatusCode::NOT_FOUND);
    }

    #[backend_test_macros::database_test]
    async fn submit_chat_empty_prompt_returns_400(db: gateway_postgres::DatabaseHandle) {
        let agent_client: SharedAgentTaskClient = Arc::new(FakeAgentTaskClient::new());
        let (db, server) = create_test_server_with_db_and_agent_client(db, agent_client).await;
        let strategy_id = insert_test_strategy(&db, "x").await;

        let res = server
            .post(&format!("/api/strategies/{strategy_id}/chat"))
            .json(&json!({ "prompt": "   " }))
            .await;
        res.assert_status(axum::http::StatusCode::BAD_REQUEST);
    }

    #[backend_test_macros::database_test]
    async fn submit_chat_agent_not_configured_returns_503(db: gateway_postgres::DatabaseHandle) {
        let fake = Arc::new(FakeAgentTaskClient::new());
        fake.set_submit_error(AgentTaskError::NotConfigured).await;
        let agent_client: SharedAgentTaskClient = fake;
        let (db, server) = create_test_server_with_db_and_agent_client(db, agent_client).await;
        let strategy_id = insert_test_strategy(&db, "x").await;
        agent_config::create(&db, DEFAULT_PURPOSE.to_string())
            .await
            .expect("insert test agent_config");

        let res = server
            .post(&format!("/api/strategies/{strategy_id}/chat"))
            .json(&json!({ "prompt": "inspect 7203" }))
            .await;
        res.assert_status(axum::http::StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(
            res.json::<serde_json::Value>(),
            json!({ "error": "agent task client is not configured" }),
        );
    }

    #[backend_test_macros::database_test]
    async fn submit_chat_missing_agent_config_uses_requested_purpose_for_status(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let agent_client: SharedAgentTaskClient = Arc::new(FakeAgentTaskClient::new());
        let (db, server) = create_test_server_with_db_and_agent_client(db, agent_client).await;
        let strategy_id = insert_test_strategy(&db, "example-strategy").await;
        let cases = [
            (
                json!({ "prompt": "inspect the example" }),
                axum::http::StatusCode::SERVICE_UNAVAILABLE,
                "agent_config for purpose 'default' not found",
            ),
            (
                json!({ "prompt": "inspect the example", "purpose": "example-purpose" }),
                axum::http::StatusCode::BAD_REQUEST,
                "agent_config for purpose 'example-purpose' not found",
            ),
        ];

        for (body, expected_status, expected_error) in cases {
            let res = server
                .post(&format!("/api/strategies/{strategy_id}/chat"))
                .json(&body)
                .await;

            assert_eq!(
                (res.status_code(), res.json::<serde_json::Value>()),
                (expected_status, json!({ "error": expected_error })),
            );
        }
    }

    #[backend_test_macros::database_test]
    async fn get_strategy_task_returns_phase(db: gateway_postgres::DatabaseHandle) {
        let agent_client: SharedAgentTaskClient = Arc::new(FakeAgentTaskClient::new());
        let (db, server) = create_test_server_with_db_and_agent_client(db, agent_client).await;
        let strategy_id = insert_test_strategy(&db, "x").await;
        agent_config::create(&db, DEFAULT_PURPOSE.to_string())
            .await
            .expect("insert test agent_config");

        let submit = server
            .post(&format!("/api/strategies/{strategy_id}/chat"))
            .json(&json!({ "prompt": "p" }))
            .await;
        submit.assert_status(axum::http::StatusCode::ACCEPTED);
        let task_id = submit.json::<serde_json::Value>()["task_id"]
            .as_str()
            .map(|s| Uuid::parse_str(s).unwrap())
            .expect("task_id");
        let a2a_task_id = submit.json::<serde_json::Value>()["a2a_task_id"]
            .as_str()
            .expect("a2a_task_id")
            .to_string();

        let res = server
            .get(&format!("/api/strategies/{strategy_id}/tasks/{task_id}"))
            .await;
        res.assert_status_ok();
        let mut body: serde_json::Value = res.json();
        strip_timestamps(&mut body);
        assert_eq!(
            body,
            json!({
                "task_id": task_id,
                "strategy_id": strategy_id,
                "a2a_task_id": a2a_task_id,
                "source": "frontend",
                "prompt": "p",
                "phase": "running",
                "error_summary": null,
                "result_text": null,
                "steps": [],
                "purpose": "default",
            }),
        );
    }

    #[backend_test_macros::database_test]
    async fn get_strategy_task_unknown_returns_404(db: gateway_postgres::DatabaseHandle) {
        let agent_client: SharedAgentTaskClient = Arc::new(FakeAgentTaskClient::new());
        let (db, server) = create_test_server_with_db_and_agent_client(db, agent_client).await;
        let strategy_id = insert_test_strategy(&db, "x").await;

        let res = server
            .get(&format!(
                "/api/strategies/{strategy_id}/tasks/00000000-0000-0000-0000-000000000000"
            ))
            .await;
        res.assert_status(axum::http::StatusCode::NOT_FOUND);
    }

    #[backend_test_macros::database_test]
    async fn get_strategy_task_strategy_mismatch_returns_404(db: gateway_postgres::DatabaseHandle) {
        let agent_client: SharedAgentTaskClient = Arc::new(FakeAgentTaskClient::new());
        let (db, server) = create_test_server_with_db_and_agent_client(db, agent_client).await;
        let strategy_a = insert_test_strategy(&db, "a").await;
        let strategy_b = insert_test_strategy(&db, "b").await;
        agent_config::create(&db, DEFAULT_PURPOSE.to_string())
            .await
            .expect("insert test agent_config");

        let submit = server
            .post(&format!("/api/strategies/{strategy_a}/chat"))
            .json(&json!({ "prompt": "p" }))
            .await;
        submit.assert_status(axum::http::StatusCode::ACCEPTED);
        let task_id = submit.json::<serde_json::Value>()["task_id"]
            .as_str()
            .map(|s| Uuid::parse_str(s).unwrap())
            .expect("task_id");

        let res = server
            .get(&format!("/api/strategies/{strategy_b}/tasks/{task_id}"))
            .await;
        res.assert_status(axum::http::StatusCode::NOT_FOUND);
    }

    #[backend_test_macros::database_test]
    async fn list_strategy_tasks_returns_tasks_newest_first(db: gateway_postgres::DatabaseHandle) {
        let (db, server) = create_test_server_with_db(db).await;
        let strategy_id = insert_test_strategy(&db, "x").await;

        let base = chrono::Utc::now().fixed_offset();
        let task1 = insert_test_strategy_task(&db, strategy_id, "first", None, base).await;
        let task2 = insert_test_strategy_task(
            &db,
            strategy_id,
            "second",
            None,
            base + chrono::Duration::seconds(1),
        )
        .await;
        let task3 = insert_test_strategy_task(
            &db,
            strategy_id,
            "third",
            None,
            base + chrono::Duration::seconds(2),
        )
        .await;

        let res = server
            .get(&format!("/api/strategies/{strategy_id}/tasks"))
            .await;
        res.assert_status_ok();
        let mut body: Vec<serde_json::Value> = res.json();
        body.iter_mut().for_each(strip_timestamps);
        assert_eq!(
            body,
            vec![
                json!({
                    "task_id": task3,
                    "strategy_id": strategy_id,
                    "source": "frontend",
                    "prompt": "third",
                    "phase": "completed",
                    "error_summary": null,
                    "purpose": null,
                }),
                json!({
                    "task_id": task2,
                    "strategy_id": strategy_id,
                    "source": "frontend",
                    "prompt": "second",
                    "phase": "completed",
                    "error_summary": null,
                    "purpose": null,
                }),
                json!({
                    "task_id": task1,
                    "strategy_id": strategy_id,
                    "source": "frontend",
                    "prompt": "first",
                    "phase": "completed",
                    "error_summary": null,
                    "purpose": null,
                }),
            ],
        );
    }

    #[backend_test_macros::database_test]
    async fn list_strategy_tasks_unknown_strategy_returns_404(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let server = create_test_server(db).await;
        let res = server
            .get("/api/strategies/00000000-0000-0000-0000-000000000000/tasks")
            .await;
        res.assert_status(axum::http::StatusCode::NOT_FOUND);
    }

    #[backend_test_macros::database_test]
    async fn list_strategy_tasks_scoped_to_strategy(db: gateway_postgres::DatabaseHandle) {
        let (db, server) = create_test_server_with_db(db).await;
        let strategy_a = insert_test_strategy(&db, "a").await;
        let strategy_b = insert_test_strategy(&db, "b").await;

        let base = chrono::Utc::now().fixed_offset();
        let task_a = insert_test_strategy_task(&db, strategy_a, "for-a", None, base).await;
        insert_test_strategy_task(&db, strategy_b, "for-b", None, base).await;

        let res = server
            .get(&format!("/api/strategies/{strategy_a}/tasks"))
            .await;
        res.assert_status_ok();
        let mut body: Vec<serde_json::Value> = res.json();
        body.iter_mut().for_each(strip_timestamps);
        assert_eq!(
            body,
            vec![json!({
                "task_id": task_a,
                "strategy_id": strategy_a,
                "source": "frontend",
                "prompt": "for-a",
                "phase": "completed",
                "error_summary": null,
                "purpose": null,
            })],
        );
    }
}
