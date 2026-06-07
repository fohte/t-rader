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
use crate::models::{ChangeStatusRequest, CreateAnnotationRequest, UpdateAnnotationRequest};
use crate::services::change_history::{self, Op, TargetKind};

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
    let txn = state.db.begin().await?;
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
    // 同 status への遷移は no-op。履歴ノイズを避けるため現在の row をそのまま返す。
    if current.status == new_status {
        return Ok(current);
    }
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
    )
)]
pub async fn reject_annotation(
    State(state): State<AppState>,
    JsonPath(id): JsonPath<Uuid>,
    JsonBody(payload): JsonBody<ChangeStatusRequest>,
) -> Result<Json<annotation::Model>, AppError> {
    Ok(Json(
        change_annotation_status(&state, id, "rejected", payload.label).await?,
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
