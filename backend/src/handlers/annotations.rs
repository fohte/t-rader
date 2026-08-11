use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use sea_orm::ActiveModelTrait;
use sea_orm::ActiveValue::{NotSet, Set};
use sea_orm::{
    ColumnTrait, EntityTrait, IntoActiveModel, QueryFilter, QueryOrder, TransactionTrait,
};
use serde::Deserialize;
use serde_json::json;
use utoipa::IntoParams;
use uuid::Uuid;

use crate::AppState;
use crate::entities::annotation;
use crate::error::{AppError, ErrorResponse};
use crate::extractors::{JsonBody, JsonPath, JsonQuery};
use crate::handlers::strategies::map_submit_error;
use crate::models::{ChangeStatusRequest, CreateAnnotationRequest, UpdateAnnotationRequest};
use crate::services::change_history::{self, Op, TargetKind};
use crate::services::strategies::ensure_strategy_exists;
use crate::services::strategy_tasks::{self, TaskSource};

const ALLOWED_TARGET_KIND: [&str; 4] = ["signal", "level", "observation", "other"];
const ALLOWED_STATUS: [&str; 3] = ["approved", "unread", "rejected"];
const ALLOWED_CREATED_BY: [&str; 2] = ["human", "llm"];

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ListAnnotationsQuery {
    pub strategy_id: Option<Uuid>,
    pub target_symbol: Option<String>,
}

/// アノテーション一覧
#[utoipa::path(
    get,
    path = "/api/annotations",
    tag = "annotations",
    params(ListAnnotationsQuery),
    responses(
        (status = 200, body = Vec<annotation::Model>),
        (status = 400, description = "リクエストパラメータが不正", body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn list_annotations(
    State(state): State<AppState>,
    JsonQuery(params): JsonQuery<ListAnnotationsQuery>,
) -> Result<Json<Vec<annotation::Model>>, AppError> {
    let mut q = annotation::Entity::find().order_by_desc(annotation::Column::Timestamp);
    if let Some(sid) = params.strategy_id {
        q = q.filter(annotation::Column::StrategyId.eq(sid));
    }
    if let Some(sym) = params.target_symbol.as_deref().filter(|s| !s.is_empty()) {
        q = q.filter(annotation::Column::TargetSymbol.eq(sym));
    }
    Ok(Json(q.all(&state.db).await?))
}

/// アノテーション取得
#[utoipa::path(
    get,
    path = "/api/annotations/{id}",
    tag = "annotations",
    params(("id" = Uuid, Path, description = "アノテーション ID")),
    responses(
        (status = 200, body = annotation::Model),
        (status = 400, description = "リクエストパラメータが不正", body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn get_annotation(
    State(state): State<AppState>,
    JsonPath(id): JsonPath<Uuid>,
) -> Result<Json<annotation::Model>, AppError> {
    let m = annotation::Entity::find_by_id(id)
        .one(&state.db)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("annotation {id} not found")))?;
    Ok(Json(m))
}

/// アノテーション作成
#[utoipa::path(
    post,
    path = "/api/annotations",
    tag = "annotations",
    request_body = CreateAnnotationRequest,
    responses(
        (status = 201, body = annotation::Model),
        (status = 400, description = "リクエストパラメータが不正", body = ErrorResponse),
        (status = 422, description = "リクエストボディのパースに失敗", body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn create_annotation(
    State(state): State<AppState>,
    JsonBody(p): JsonBody<CreateAnnotationRequest>,
) -> Result<(StatusCode, Json<annotation::Model>), AppError> {
    let target_symbol = p.target_symbol.trim().to_string();
    if target_symbol.is_empty() {
        return Err(AppError::Validation(
            "target_symbol must not be empty".into(),
        ));
    }
    if !ALLOWED_TARGET_KIND.contains(&p.target_kind.as_str()) {
        return Err(AppError::Validation(format!(
            "invalid target_kind: {}",
            p.target_kind
        )));
    }
    let status = p.status.as_deref().unwrap_or("unread").to_string();
    if !ALLOWED_STATUS.contains(&status.as_str()) {
        return Err(AppError::Validation(format!("invalid status: {status}")));
    }
    let created_by = p.created_by_kind.as_deref().unwrap_or("human").to_string();
    if !ALLOWED_CREATED_BY.contains(&created_by.as_str()) {
        return Err(AppError::Validation(format!(
            "invalid created_by_kind: {created_by}"
        )));
    }

    let txn = state.db.begin().await?;
    ensure_strategy_exists(&txn, p.strategy_id).await?;

    let id = Uuid::new_v4();
    let model = annotation::ActiveModel {
        id: Set(id),
        strategy_id: Set(p.strategy_id),
        target_symbol: Set(target_symbol.clone()),
        target_kind: Set(p.target_kind.clone()),
        timestamp: Set(p.timestamp),
        price: Set(p.price),
        text: Set(p.text.clone()),
        status: Set(status),
        linked_note_id: Set(p.linked_note_id),
        created_by_kind: Set(created_by),
        created_at: NotSet,
        updated_at: NotSet,
    };
    let created = annotation::Entity::insert(model)
        .exec_with_returning(&txn)
        .await?;
    change_history::record(
        &txn,
        TargetKind::Annotation,
        id,
        Op::Create,
        json!({ "strategy_id": p.strategy_id, "target_symbol": target_symbol }),
        None,
    )
    .await?;
    txn.commit().await?;

    Ok((StatusCode::CREATED, Json(created)))
}

/// アノテーション更新
#[utoipa::path(
    patch,
    path = "/api/annotations/{id}",
    tag = "annotations",
    params(("id" = Uuid, Path, description = "アノテーション ID")),
    request_body = UpdateAnnotationRequest,
    responses(
        (status = 200, body = annotation::Model),
        (status = 400, description = "リクエストパラメータが不正", body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 422, description = "リクエストボディのパースに失敗", body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn update_annotation(
    State(state): State<AppState>,
    JsonPath(id): JsonPath<Uuid>,
    JsonBody(p): JsonBody<UpdateAnnotationRequest>,
) -> Result<Json<annotation::Model>, AppError> {
    let current = annotation::Entity::find_by_id(id)
        .one(&state.db)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("annotation {id} not found")))?;
    let mut active = current.clone().into_active_model();
    let mut diff = serde_json::Map::new();

    if let Some(v) = p.target_symbol {
        let trimmed = v.trim().to_string();
        if trimmed.is_empty() {
            return Err(AppError::Validation(
                "target_symbol must not be empty".into(),
            ));
        }
        diff.insert(
            "target_symbol".into(),
            json!({ "from": current.target_symbol, "to": trimmed }),
        );
        active.target_symbol = Set(trimmed);
    }
    if let Some(v) = p.target_kind {
        if !ALLOWED_TARGET_KIND.contains(&v.as_str()) {
            return Err(AppError::Validation(format!("invalid target_kind: {v}")));
        }
        diff.insert(
            "target_kind".into(),
            json!({ "from": current.target_kind, "to": v }),
        );
        active.target_kind = Set(v);
    }
    if let Some(v) = p.timestamp {
        diff.insert(
            "timestamp".into(),
            json!({ "from": current.timestamp, "to": v }),
        );
        active.timestamp = Set(v);
    }
    if let Some(v) = p.price {
        diff.insert("price".into(), json!({ "from": current.price, "to": v }));
        active.price = Set(Some(v));
    }
    if let Some(v) = p.text {
        diff.insert(
            "text".into(),
            json!({ "len_from": current.text.len(), "len_to": v.len() }),
        );
        active.text = Set(v);
    }
    if let Some(v) = p.linked_note_id {
        diff.insert(
            "linked_note_id".into(),
            json!({ "from": current.linked_note_id, "to": v }),
        );
        active.linked_note_id = Set(Some(v));
    }
    active.updated_at = Set(chrono::Utc::now().fixed_offset());

    let txn = state.db.begin().await?;
    let updated = active.update(&txn).await?;
    if !diff.is_empty() {
        change_history::record(
            &txn,
            TargetKind::Annotation,
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

async fn change_annotation_status_from(
    state: &AppState,
    current: annotation::Model,
    new_status: &str,
    label: Option<String>,
) -> Result<annotation::Model, AppError> {
    if current.status == new_status {
        return Ok(current);
    }
    let id = current.id;
    let mut active = current.clone().into_active_model();
    active.status = Set(new_status.to_string());
    active.updated_at = Set(chrono::Utc::now().fixed_offset());
    let txn = state.db.begin().await?;
    let updated = active.update(&txn).await?;
    change_history::record(
        &txn,
        TargetKind::Annotation,
        id,
        Op::StatusChange,
        json!({ "from": current.status, "to": new_status, "label": label }),
        label,
    )
    .await?;
    txn.commit().await?;
    Ok(updated)
}

async fn change_annotation_status(
    state: &AppState,
    id: Uuid,
    new_status: &str,
    label: Option<String>,
) -> Result<annotation::Model, AppError> {
    let current = annotation::Entity::find_by_id(id)
        .one(&state.db)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("annotation {id} not found")))?;
    change_annotation_status_from(state, current, new_status, label).await
}

/// アノテーションを approved に遷移
#[utoipa::path(
    post,
    path = "/api/annotations/{id}/approve",
    tag = "annotations",
    params(("id" = Uuid, Path, description = "アノテーション ID")),
    request_body = ChangeStatusRequest,
    responses(
        (status = 200, body = annotation::Model),
        (status = 400, description = "リクエストパラメータが不正", body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 422, description = "リクエストボディのパースに失敗", body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn approve_annotation(
    State(state): State<AppState>,
    JsonPath(id): JsonPath<Uuid>,
    JsonBody(payload): JsonBody<ChangeStatusRequest>,
) -> Result<Json<annotation::Model>, AppError> {
    Ok(Json(
        change_annotation_status(&state, id, "approved", payload.label).await?,
    ))
}

/// アノテーションを rejected に遷移
#[utoipa::path(
    post,
    path = "/api/annotations/{id}/reject",
    tag = "annotations",
    params(("id" = Uuid, Path, description = "アノテーション ID")),
    request_body = ChangeStatusRequest,
    responses(
        (status = 200, body = annotation::Model),
        (status = 400, description = "リクエストパラメータが不正", body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 422, description = "リクエストボディのパースに失敗", body = ErrorResponse),
        (status = 500, body = ErrorResponse),
        (status = 503, description = "agent task client が未設定", body = ErrorResponse),
    )
)]
pub async fn reject_annotation(
    State(state): State<AppState>,
    JsonPath(id): JsonPath<Uuid>,
    JsonBody(payload): JsonBody<ChangeStatusRequest>,
) -> Result<Json<annotation::Model>, AppError> {
    let current = annotation::Entity::find_by_id(id)
        .one(&state.db)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("annotation {id} not found")))?;
    // 却下確定前の check-then-act。ほぼ同時に reject が 2 回届くと両方通過し得るが、
    // frontend は mutation pending 中ボタンを disable するため実運用では起きない。
    if current.status == "rejected" {
        return Ok(Json(current));
    }

    let prompt = format!(
        "アノテーション (id: {}, 対象: {}) がレビューで却下されました。付いているコメントを確認し、指摘を反映してください。",
        current.id, current.target_symbol
    );
    strategy_tasks::submit_task(
        &state.db,
        &state.agent_task_client,
        current.strategy_id,
        &prompt,
        TaskSource::Review,
    )
    .await
    .map_err(map_submit_error)?;

    Ok(Json(
        change_annotation_status_from(&state, current, "rejected", payload.label).await?,
    ))
}

/// アノテーション削除
#[utoipa::path(
    delete,
    path = "/api/annotations/{id}",
    tag = "annotations",
    params(("id" = Uuid, Path, description = "アノテーション ID")),
    responses(
        (status = 204),
        (status = 400, description = "リクエストパラメータが不正", body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn delete_annotation(
    State(state): State<AppState>,
    JsonPath(id): JsonPath<Uuid>,
) -> Result<StatusCode, AppError> {
    let txn = state.db.begin().await?;
    let res = annotation::Entity::delete_by_id(id).exec(&txn).await?;
    if res.rows_affected == 0 {
        return Err(AppError::NotFound(format!("annotation {id} not found")));
    }
    change_history::record(
        &txn,
        TargetKind::Annotation,
        id,
        Op::Delete,
        json!({}),
        None,
    )
    .await?;
    txn.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use axum_test::TestServer;
    use serde_json::Value;
    use sqlx::PgPool;

    use super::*;
    use crate::agent_client::{AgentTaskError, FakeAgentTaskClient, SharedAgentTaskClient};
    use crate::entities::sea_orm_active_enums::StrategyTaskPhase;
    use crate::entities::strategy_task;
    use crate::testing::{create_test_server_with_db_and_agent_client, insert_test_strategy};

    /// strategy_task 行の動的フィールド (id / 時刻 / a2a_task_id) を捨てた比較用ビュー。
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

    async fn create_test_annotation(server: &TestServer, strategy_id: Uuid) -> Uuid {
        let res = server
            .post("/api/annotations")
            .json(&json!({
                "strategy_id": strategy_id,
                "target_symbol": "7203",
                "target_kind": "observation",
                "timestamp": "2026-01-01T00:00:00Z",
                "text": "text",
            }))
            .await;
        res.assert_status(StatusCode::CREATED);
        let body: Value = res.json();
        Uuid::parse_str(body["id"].as_str().expect("id")).expect("uuid")
    }

    #[sqlx::test(migrations = false)]
    async fn reject_annotation_submits_single_review_task_referencing_annotation(pool: PgPool) {
        let fake = Arc::new(FakeAgentTaskClient::new());
        let agent_client: SharedAgentTaskClient = fake.clone();
        let (db, server) = create_test_server_with_db_and_agent_client(pool, agent_client).await;
        let strategy_id = insert_test_strategy(&db, "s").await;
        let anno_id = create_test_annotation(&server, strategy_id).await;

        let res = server
            .post(&format!("/api/annotations/{anno_id}/reject"))
            .json(&json!({}))
            .await;
        res.assert_status_ok();
        let mut body: Value = res.json();
        let obj = body.as_object_mut().unwrap();
        obj.remove("created_at");
        obj.remove("updated_at");
        assert_eq!(
            body,
            json!({
                "id": anno_id,
                "strategy_id": strategy_id,
                "target_symbol": "7203",
                "target_kind": "observation",
                "timestamp": "2026-01-01T00:00:00Z",
                "price": null,
                "text": "text",
                "status": "rejected",
                "linked_note_id": null,
                "created_by_kind": "human",
            }),
        );

        let tasks = strategy_task::Entity::find()
            .filter(strategy_task::Column::StrategyId.eq(strategy_id))
            .all(&db)
            .await
            .unwrap();
        assert_eq!(
            tasks.iter().map(TaskShape::from).collect::<Vec<_>>(),
            vec![TaskShape {
                strategy_id,
                source: "review".to_string(),
                prompt: format!(
                    "アノテーション (id: {anno_id}, 対象: 7203) がレビューで却下されました。\
付いているコメントを確認し、指摘を反映してください。"
                ),
                phase: StrategyTaskPhase::Running,
            }],
        );
    }

    #[sqlx::test(migrations = false)]
    async fn rejecting_already_rejected_annotation_does_not_resubmit(pool: PgPool) {
        let fake = Arc::new(FakeAgentTaskClient::new());
        let agent_client: SharedAgentTaskClient = fake.clone();
        let (db, server) = create_test_server_with_db_and_agent_client(pool, agent_client).await;
        let strategy_id = insert_test_strategy(&db, "s").await;
        let anno_id = create_test_annotation(&server, strategy_id).await;

        for _ in 0..2 {
            let res = server
                .post(&format!("/api/annotations/{anno_id}/reject"))
                .json(&json!({}))
                .await;
            res.assert_status_ok();
        }

        let tasks = strategy_task::Entity::find()
            .filter(strategy_task::Column::StrategyId.eq(strategy_id))
            .all(&db)
            .await
            .unwrap();
        assert_eq!(tasks.len(), 1);
    }

    #[sqlx::test(migrations = false)]
    async fn reject_annotation_leaves_status_unchanged_when_agent_submission_fails(pool: PgPool) {
        let fake = Arc::new(FakeAgentTaskClient::new());
        fake.set_submit_error(AgentTaskError::NotConfigured).await;
        let agent_client: SharedAgentTaskClient = fake;
        let (db, server) = create_test_server_with_db_and_agent_client(pool, agent_client).await;
        let strategy_id = insert_test_strategy(&db, "s").await;
        let anno_id = create_test_annotation(&server, strategy_id).await;

        let res = server
            .post(&format!("/api/annotations/{anno_id}/reject"))
            .json(&json!({}))
            .await;
        res.assert_status(StatusCode::SERVICE_UNAVAILABLE);

        let anno = annotation::Entity::find_by_id(anno_id)
            .one(&db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(anno.status, "unread");
    }
}
