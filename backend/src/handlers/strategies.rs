use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use sea_orm::ActiveModelTrait;
use sea_orm::ActiveValue::{NotSet, Set};
use sea_orm::{
    ColumnTrait, EntityTrait, IntoActiveModel, QueryFilter, QueryOrder, TransactionTrait,
};
use serde_json::json;
use uuid::Uuid;

use crate::AppState;
use crate::entities::{strategy, strategy_interest};
use crate::error::{AppError, ErrorResponse};
use crate::extractors::{JsonBody, JsonPath};
use crate::kubeopencode::{KubeopencodeError, agent_name_for};
use crate::models::{
    CreateStrategyRequest, StrategyChatRequest, StrategyChatResponse, StrategyTaskStatusResponse,
    UpdateStrategyRequest,
};
use crate::services::change_history::{self, Op, TargetKind};
use crate::services::strategy_agent;
use crate::services::strategy_tasks::{self, GetTaskError, SubmitTaskError, TaskSource, phase_str};

fn validate_name(value: &str) -> Result<String, AppError> {
    let trimmed = value.trim().to_string();
    if trimmed.is_empty() {
        return Err(AppError::Validation("name must not be empty".into()));
    }
    Ok(trimmed)
}

async fn find_strategy_or_404(
    db: &sea_orm::DatabaseConnection,
    id: Uuid,
) -> Result<strategy::Model, AppError> {
    strategy::Entity::find_by_id(id)
        .one(db)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("strategy {id} not found")))
}

/// 戦略一覧
#[utoipa::path(
    get,
    path = "/api/strategies",
    tag = "strategies",
    responses(
        (status = 200, description = "戦略一覧", body = Vec<strategy::Model>),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn list_strategies(
    State(state): State<AppState>,
) -> Result<Json<Vec<strategy::Model>>, AppError> {
    let items = strategy::Entity::find()
        .order_by_asc(strategy::Column::SortOrder)
        .order_by_asc(strategy::Column::CreatedAt)
        .all(&state.db)
        .await?;
    Ok(Json(items))
}

/// 戦略取得
#[utoipa::path(
    get,
    path = "/api/strategies/{id}",
    tag = "strategies",
    params(("id" = Uuid, Path, description = "戦略 ID")),
    responses(
        (status = 200, body = strategy::Model),
        (status = 400, description = "リクエストパラメータが不正", body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn get_strategy(
    State(state): State<AppState>,
    JsonPath(id): JsonPath<Uuid>,
) -> Result<Json<strategy::Model>, AppError> {
    let model = find_strategy_or_404(&state.db, id).await?;
    Ok(Json(model))
}

/// 戦略作成
#[utoipa::path(
    post,
    path = "/api/strategies",
    tag = "strategies",
    request_body = CreateStrategyRequest,
    responses(
        (status = 201, body = strategy::Model),
        (status = 400, description = "リクエストパラメータが不正", body = ErrorResponse),
        (status = 422, description = "リクエストボディのパースに失敗", body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn create_strategy(
    State(state): State<AppState>,
    JsonBody(payload): JsonBody<CreateStrategyRequest>,
) -> Result<(StatusCode, Json<strategy::Model>), AppError> {
    let name = validate_name(&payload.name)?;
    let id = Uuid::new_v4();
    let sort_order = payload.sort_order.unwrap_or(0);

    let model = strategy::ActiveModel {
        id: Set(id),
        name: Set(name.clone()),
        description: Set(payload.description.clone()),
        sort_order: Set(sort_order),
        agents_md: NotSet,
        skills: NotSet,
        agent_status: NotSet,
        agent_error: NotSet,
        created_at: NotSet,
        updated_at: NotSet,
    };
    let txn = state.db.begin().await?;
    let created = strategy::Entity::insert(model)
        .exec_with_returning(&txn)
        .await?;
    change_history::record(
        &txn,
        TargetKind::Strategy,
        id,
        Op::Create,
        json!({ "name": name, "sort_order": sort_order }),
        Some(format!("created strategy {name}")),
    )
    .await?;
    txn.commit().await?;

    // commit 前に spawn すると rollback 時に Agent CR が孤立する
    strategy_agent::spawn_reconcile(state.db.clone(), state.kubeopencode.clone(), id);

    Ok((StatusCode::CREATED, Json(created)))
}

/// 戦略更新
#[utoipa::path(
    patch,
    path = "/api/strategies/{id}",
    tag = "strategies",
    params(("id" = Uuid, Path, description = "戦略 ID")),
    request_body = UpdateStrategyRequest,
    responses(
        (status = 200, body = strategy::Model),
        (status = 400, description = "リクエストパラメータが不正", body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 422, description = "リクエストボディのパースに失敗", body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn update_strategy(
    State(state): State<AppState>,
    JsonPath(id): JsonPath<Uuid>,
    JsonBody(payload): JsonBody<UpdateStrategyRequest>,
) -> Result<Json<strategy::Model>, AppError> {
    let current = find_strategy_or_404(&state.db, id).await?;
    let mut active = current.clone().into_active_model();
    let mut diff = serde_json::Map::new();

    if let Some(name) = payload.name.as_ref() {
        let name = validate_name(name)?;
        diff.insert("name".into(), json!({ "from": current.name, "to": name }));
        active.name = Set(name);
    }
    if let Some(description) = payload.description.as_ref() {
        diff.insert(
            "description".into(),
            json!({ "from": current.description, "to": description }),
        );
        active.description = Set(Some(description.clone()));
    }
    if let Some(sort_order) = payload.sort_order {
        diff.insert(
            "sort_order".into(),
            json!({ "from": current.sort_order, "to": sort_order }),
        );
        active.sort_order = Set(sort_order);
    }
    active.updated_at = Set(chrono::Utc::now().fixed_offset());

    let txn = state.db.begin().await?;
    let updated = active.update(&txn).await?;
    if !diff.is_empty() {
        change_history::record(
            &txn,
            TargetKind::Strategy,
            id,
            Op::Update,
            serde_json::Value::Object(diff),
            None,
        )
        .await?;
    }
    txn.commit().await?;

    Ok(Json(updated))
}

/// 戦略削除
#[utoipa::path(
    delete,
    path = "/api/strategies/{id}",
    tag = "strategies",
    params(("id" = Uuid, Path, description = "戦略 ID")),
    responses(
        (status = 204),
        (status = 400, description = "リクエストパラメータが不正", body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn delete_strategy(
    State(state): State<AppState>,
    JsonPath(id): JsonPath<Uuid>,
) -> Result<StatusCode, AppError> {
    // 不在の id なら Kube API を叩く前に 404 を確定させる
    find_strategy_or_404(&state.db, id).await?;

    // Agent CR を先に消す: DB を先に消すと孤児 Agent が一時的に残り、in-flight Task の
    // agentRef は解決できるが履歴 (strategy_task / change_history) との突き合わせが切れる
    let agent_name = agent_name_for(id);
    match state.kubeopencode.delete_strategy_agent(&agent_name).await {
        Ok(()) | Err(KubeopencodeError::NotConfigured) => {}
        Err(err) => {
            tracing::error!(error = %err, strategy_id = %id, "failed to delete strategy agent");
            return Err(AppError::Config(format!(
                "failed to delete strategy agent: {err}"
            )));
        }
    }

    let txn = state.db.begin().await?;
    let result = strategy::Entity::delete_by_id(id).exec(&txn).await?;
    if result.rows_affected == 0 {
        return Err(AppError::NotFound(format!("strategy {id} not found")));
    }
    change_history::record(&txn, TargetKind::Strategy, id, Op::Delete, json!({}), None).await?;
    txn.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

/// 戦略の関心 (シード + LLM 派生) 一覧
#[utoipa::path(
    get,
    path = "/api/strategies/{id}/interests",
    tag = "strategies",
    params(("id" = Uuid, Path, description = "戦略 ID")),
    responses(
        (status = 200, body = Vec<strategy_interest::Model>),
        (status = 400, description = "リクエストパラメータが不正", body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn list_strategy_interests(
    State(state): State<AppState>,
    JsonPath(id): JsonPath<Uuid>,
) -> Result<Json<Vec<strategy_interest::Model>>, AppError> {
    find_strategy_or_404(&state.db, id).await?;
    let items = strategy_interest::Entity::find()
        .filter(strategy_interest::Column::StrategyId.eq(id))
        .order_by_asc(strategy_interest::Column::Role)
        .order_by_asc(strategy_interest::Column::CreatedAt)
        .all(&state.db)
        .await?;
    Ok(Json(items))
}

fn map_submit_error(err: SubmitTaskError) -> AppError {
    match err {
        SubmitTaskError::EmptyPrompt => AppError::Validation("prompt must not be empty".into()),
        SubmitTaskError::StrategyNotFound(id) => {
            AppError::NotFound(format!("strategy {id} not found"))
        }
        SubmitTaskError::AgentPending => AppError::ServiceUnavailable(
            "strategy agent not ready: status=pending (reconcile in progress)".into(),
        ),
        SubmitTaskError::AgentFailed(reason) => AppError::ServiceUnavailable(format!(
            "strategy agent not ready: status=failed: {reason}"
        )),
        SubmitTaskError::Database(db_err) => AppError::Database(db_err),
        SubmitTaskError::Kubeopencode(kube_err) => match kube_err {
            KubeopencodeError::AlreadyExists(name) => {
                AppError::Conflict(format!("kubeopencode task already exists: {name}"))
            }
            other => AppError::Config(format!("kubeopencode error: {other}")),
        },
    }
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
        (status = 400, description = "prompt が空 (空白のみを含む)", body = ErrorResponse),
        (status = 404, description = "戦略が存在しない", body = ErrorResponse),
        (status = 409, description = "kubeopencode タスクの名前衝突", body = ErrorResponse),
        (status = 422, description = "リクエストボディのパースに失敗", body = ErrorResponse),
        (status = 503, description = "戦略 Agent が ready ではない", body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn submit_strategy_chat(
    State(state): State<AppState>,
    JsonPath(id): JsonPath<Uuid>,
    JsonBody(payload): JsonBody<StrategyChatRequest>,
) -> Result<(StatusCode, Json<StrategyChatResponse>), AppError> {
    let submitted = strategy_tasks::submit_task(
        &state.db,
        &state.kubeopencode,
        id,
        &payload.prompt,
        TaskSource::Frontend,
    )
    .await
    .map_err(map_submit_error)?;
    Ok((
        StatusCode::ACCEPTED,
        Json(StrategyChatResponse {
            task_id: submitted.task_id,
            kubeopencode_task_name: submitted.kubeopencode_task_name,
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
    let view = strategy_tasks::get_task_for_strategy(&state.db, strategy_id, task_id)
        .await
        .map_err(|err| match err {
            GetTaskError::NotFound(id) => {
                AppError::NotFound(format!("strategy task {id} not found"))
            }
            GetTaskError::StrategyMismatch { task_id, .. } => {
                AppError::NotFound(format!("strategy task {task_id} not found"))
            }
            GetTaskError::Database(db_err) => AppError::Database(db_err),
        })?;
    Ok(Json(StrategyTaskStatusResponse {
        task_id: view.task_id,
        strategy_id: view.strategy_id,
        kubeopencode_task_name: view.kubeopencode_task_name,
        source: view.source,
        phase: phase_str(&view.phase).to_string(),
        error_summary: view.error_summary,
        created_at: view.created_at,
        updated_at: view.updated_at,
    }))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use sea_orm::ActiveModelTrait;
    use sea_orm::ActiveValue::{NotSet, Set};
    use sea_orm::{DatabaseConnection, EntityTrait};
    use serde_json::json;
    use sqlx::PgPool;

    use uuid::Uuid;

    use crate::entities::sea_orm_active_enums::StrategyAgentStatus;
    use crate::entities::{strategy, strategy_task};
    use crate::kubeopencode::{FakeKubeopencodeClient, SharedKubeopencodeClient};
    use crate::testing::{
        create_test_server, create_test_server_with_db_and_kube, create_test_server_with_kube,
    };

    async fn insert_strategy_ready(db: &DatabaseConnection, name: &str) -> Uuid {
        insert_strategy(db, name, StrategyAgentStatus::Ready).await
    }

    async fn insert_strategy(
        db: &DatabaseConnection,
        name: &str,
        status: StrategyAgentStatus,
    ) -> Uuid {
        let id = Uuid::new_v4();
        strategy::ActiveModel {
            id: Set(id),
            name: Set(name.to_string()),
            description: Set(None),
            sort_order: Set(0),
            agents_md: NotSet,
            skills: NotSet,
            agent_status: Set(status),
            agent_error: NotSet,
            created_at: NotSet,
            updated_at: NotSet,
        }
        .insert(db)
        .await
        .expect("insert strategy");
        id
    }

    #[sqlx::test(migrations = false)]
    async fn create_and_list_strategy(pool: PgPool) {
        let server = create_test_server(pool).await;
        let res = server
            .post("/api/strategies")
            .json(&json!({ "name": "長期投資" }))
            .await;
        res.assert_status(axum::http::StatusCode::CREATED);

        let list = server.get("/api/strategies").await;
        list.assert_status_ok();
        let body: Vec<serde_json::Value> = list.json();
        assert_eq!(body.len(), 1);
        assert_eq!(body[0]["name"], "長期投資");
    }

    #[sqlx::test(migrations = false)]
    async fn get_nonexistent_strategy_returns_404(pool: PgPool) {
        let server = create_test_server(pool).await;
        let res = server
            .get("/api/strategies/00000000-0000-0000-0000-000000000000")
            .await;
        res.assert_status(axum::http::StatusCode::NOT_FOUND);
    }

    #[sqlx::test(migrations = false)]
    async fn list_interests_of_nonexistent_strategy_returns_404(pool: PgPool) {
        let server = create_test_server(pool).await;
        let res = server
            .get("/api/strategies/00000000-0000-0000-0000-000000000000/interests")
            .await;
        res.assert_status(axum::http::StatusCode::NOT_FOUND);
    }

    #[sqlx::test(migrations = false)]
    async fn delete_strategy_calls_kube_delete(pool: PgPool) {
        let fake = Arc::new(FakeKubeopencodeClient::new());
        let kube: SharedKubeopencodeClient = fake.clone();
        let server = create_test_server_with_kube(pool, kube).await;

        let created = server
            .post("/api/strategies")
            .json(&json!({ "name": "to-delete" }))
            .await;
        created.assert_status(axum::http::StatusCode::CREATED);
        let id = created.json::<serde_json::Value>()["id"]
            .as_str()
            .map(str::to_string)
            .expect("id");

        let deleted = server.delete(&format!("/api/strategies/{id}")).await;
        deleted.assert_status(axum::http::StatusCode::NO_CONTENT);

        let uuid = Uuid::parse_str(&id).unwrap();
        let expected_agent = crate::kubeopencode::agent_name_for(uuid);
        assert_eq!(
            fake.deleted_agents.lock().await.as_slice(),
            &[expected_agent],
        );
    }

    #[sqlx::test(migrations = false)]
    async fn submit_chat_creates_task_row_and_cr(pool: PgPool) {
        let fake = Arc::new(FakeKubeopencodeClient::new());
        let kube: SharedKubeopencodeClient = fake.clone();
        let (db, server) = create_test_server_with_db_and_kube(pool, kube).await;
        let strategy_id = insert_strategy_ready(&db, "long").await;

        let res = server
            .post(&format!("/api/strategies/{strategy_id}/chat"))
            .json(&json!({ "prompt": " inspect 7203 " }))
            .await;
        res.assert_status(axum::http::StatusCode::ACCEPTED);

        let mut body: serde_json::Value = res.json();
        let task_id = Uuid::parse_str(body["task_id"].as_str().expect("task_id")).expect("uuid");
        let task_name = body["kubeopencode_task_name"]
            .as_str()
            .expect("task name")
            .to_string();
        body["task_id"] = json!("<uuid>");
        body["kubeopencode_task_name"] = json!("<task-name>");
        assert_eq!(
            body,
            json!({
                "task_id": "<uuid>",
                "kubeopencode_task_name": "<task-name>",
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
            row.kubeopencode_task_name,
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
                task_name.clone(),
                "frontend".to_string(),
                "inspect 7203".to_string(),
                crate::entities::sea_orm_active_enums::StrategyTaskPhase::Pending,
                None,
            ),
        );

        let created: Vec<(String, String, String)> = fake
            .created
            .lock()
            .await
            .iter()
            .map(|s| (s.name.clone(), s.agent_name.clone(), s.description.clone()))
            .collect();
        assert_eq!(
            created,
            vec![(
                task_name,
                crate::kubeopencode::agent_name_for(strategy_id),
                "inspect 7203".to_string(),
            )],
        );
    }

    #[sqlx::test(migrations = false)]
    async fn submit_chat_unknown_strategy_returns_404(pool: PgPool) {
        let server = create_test_server(pool).await;
        let res = server
            .post("/api/strategies/00000000-0000-0000-0000-000000000000/chat")
            .json(&json!({ "prompt": "x" }))
            .await;
        res.assert_status(axum::http::StatusCode::NOT_FOUND);
    }

    async fn assert_submit_chat_not_ready_returns_503(pool: PgPool, status: StrategyAgentStatus) {
        let fake = Arc::new(FakeKubeopencodeClient::new());
        let (db, server) =
            create_test_server_with_db_and_kube(pool, fake as SharedKubeopencodeClient).await;
        let strategy_id = insert_strategy(&db, "x", status).await;

        let res = server
            .post(&format!("/api/strategies/{strategy_id}/chat"))
            .json(&json!({ "prompt": "x" }))
            .await;
        res.assert_status(axum::http::StatusCode::SERVICE_UNAVAILABLE);
    }

    #[sqlx::test(migrations = false)]
    async fn submit_chat_pending_agent_returns_503(pool: PgPool) {
        assert_submit_chat_not_ready_returns_503(pool, StrategyAgentStatus::Pending).await;
    }

    #[sqlx::test(migrations = false)]
    async fn submit_chat_failed_agent_returns_503(pool: PgPool) {
        assert_submit_chat_not_ready_returns_503(pool, StrategyAgentStatus::Failed).await;
    }

    #[sqlx::test(migrations = false)]
    async fn submit_chat_empty_prompt_returns_400(pool: PgPool) {
        let fake = Arc::new(FakeKubeopencodeClient::new());
        let (db, server) =
            create_test_server_with_db_and_kube(pool, fake as SharedKubeopencodeClient).await;
        let strategy_id = insert_strategy_ready(&db, "x").await;

        let res = server
            .post(&format!("/api/strategies/{strategy_id}/chat"))
            .json(&json!({ "prompt": "   " }))
            .await;
        res.assert_status(axum::http::StatusCode::BAD_REQUEST);
    }

    #[sqlx::test(migrations = false)]
    async fn get_strategy_task_returns_phase(pool: PgPool) {
        let fake = Arc::new(FakeKubeopencodeClient::new());
        let (db, server) = create_test_server_with_db_and_kube(pool, fake).await;
        let strategy_id = insert_strategy_ready(&db, "x").await;

        let submit = server
            .post(&format!("/api/strategies/{strategy_id}/chat"))
            .json(&json!({ "prompt": "p" }))
            .await;
        submit.assert_status(axum::http::StatusCode::ACCEPTED);
        let task_id = submit.json::<serde_json::Value>()["task_id"]
            .as_str()
            .map(|s| Uuid::parse_str(s).unwrap())
            .expect("task_id");
        let task_name = submit.json::<serde_json::Value>()["kubeopencode_task_name"]
            .as_str()
            .expect("task name")
            .to_string();

        let res = server
            .get(&format!("/api/strategies/{strategy_id}/tasks/{task_id}"))
            .await;
        res.assert_status_ok();
        let body: serde_json::Value = res.json();
        let mut normalized = body.clone();
        let obj = normalized.as_object_mut().unwrap();
        obj.remove("created_at");
        obj.remove("updated_at");
        assert_eq!(
            normalized,
            json!({
                "task_id": task_id,
                "strategy_id": strategy_id,
                "kubeopencode_task_name": task_name,
                "source": "frontend",
                "phase": "pending",
                "error_summary": null,
            }),
        );
    }

    #[sqlx::test(migrations = false)]
    async fn get_strategy_task_unknown_returns_404(pool: PgPool) {
        let fake = Arc::new(FakeKubeopencodeClient::new());
        let (db, server) =
            create_test_server_with_db_and_kube(pool, fake as SharedKubeopencodeClient).await;
        let strategy_id = insert_strategy_ready(&db, "x").await;

        let res = server
            .get(&format!(
                "/api/strategies/{strategy_id}/tasks/00000000-0000-0000-0000-000000000000"
            ))
            .await;
        res.assert_status(axum::http::StatusCode::NOT_FOUND);
    }

    #[sqlx::test(migrations = false)]
    async fn get_strategy_task_strategy_mismatch_returns_404(pool: PgPool) {
        let fake = Arc::new(FakeKubeopencodeClient::new());
        let (db, server) = create_test_server_with_db_and_kube(pool, fake).await;
        let strategy_a = insert_strategy_ready(&db, "a").await;
        let strategy_b = insert_strategy_ready(&db, "b").await;

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

    #[sqlx::test(migrations = false)]
    async fn list_interests_of_existing_strategy_returns_empty(pool: PgPool) {
        let server = create_test_server(pool).await;
        let created = server
            .post("/api/strategies")
            .json(&json!({ "name": "test" }))
            .await;
        created.assert_status(axum::http::StatusCode::CREATED);
        let id = created.json::<serde_json::Value>()["id"]
            .as_str()
            .map(str::to_string)
            .expect("id");

        let res = server.get(&format!("/api/strategies/{id}/interests")).await;
        res.assert_status_ok();
        let body: Vec<serde_json::Value> = res.json();
        assert!(body.is_empty());
    }
}
