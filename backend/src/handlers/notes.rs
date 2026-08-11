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
use crate::entities::{note, note_ref};
use crate::error::{AppError, ErrorResponse};
use crate::extractors::{JsonBody, JsonPath, JsonQuery};
use crate::handlers::strategies::map_submit_error;
use crate::models::{ChangeStatusRequest, CreateNoteRequest, UpdateNoteRequest};
use crate::services::change_history::{self, Op, TargetKind};
use crate::services::strategies::ensure_strategy_exists;
use crate::services::strategy_tasks::{self, TaskSource};

const ALLOWED_STATUSES: [&str; 3] = ["approved", "unread", "rejected"];
const ALLOWED_CREATED_BY: [&str; 2] = ["human", "llm"];
const ALLOWED_REF_KINDS: [&str; 4] = ["stock", "indicator", "sector", "theme"];

fn ensure_frontmatter_object(fm: &serde_json::Value) -> Result<(), AppError> {
    if fm.is_object() {
        Ok(())
    } else {
        Err(AppError::Validation(
            "frontmatter_json must be a JSON object".into(),
        ))
    }
}

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ListNotesQuery {
    pub strategy_id: Option<Uuid>,
    pub status: Option<String>,
    pub type_tag: Option<String>,
}

async fn find_note_or_404(
    db: &sea_orm::DatabaseConnection,
    id: Uuid,
) -> Result<note::Model, AppError> {
    note::Entity::find_by_id(id)
        .one(db)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("note {id} not found")))
}

/// note_ref を本文から都度 rebuild する: 旧 ref は DELETE で消え、本文に残るものだけ INSERT 復元する
async fn sync_note_refs<C: sea_orm::ConnectionTrait>(
    db: &C,
    note_id: Uuid,
    body_md: &str,
) -> Result<(), AppError> {
    let mut refs = extract_refs(body_md);
    refs.sort();
    refs.dedup();

    note_ref::Entity::delete_many()
        .filter(note_ref::Column::NoteId.eq(note_id))
        .exec(db)
        .await?;

    if refs.is_empty() {
        return Ok(());
    }

    let models: Vec<note_ref::ActiveModel> = refs
        .into_iter()
        .map(|(kind, id)| note_ref::ActiveModel {
            note_id: Set(note_id),
            ref_kind: Set(kind),
            ref_id: Set(id),
        })
        .collect();

    note_ref::Entity::insert_many(models)
        .on_conflict(
            sea_orm::sea_query::OnConflict::columns([
                note_ref::Column::NoteId,
                note_ref::Column::RefKind,
                note_ref::Column::RefId,
            ])
            .do_nothing()
            .to_owned(),
        )
        .exec_without_returning(db)
        .await?;
    Ok(())
}

fn extract_refs(body: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut rest = body;
    while let Some(start) = rest.find("[[") {
        rest = &rest[start + 2..];
        let Some(end) = rest.find("]]") else { break };
        let inner = &rest[..end];
        if let Some((kind, id)) = inner.split_once(':') {
            let kind = kind.trim();
            let id = id.trim();
            if ALLOWED_REF_KINDS.contains(&kind) && !id.is_empty() {
                out.push((kind.to_string(), id.to_string()));
            }
        }
        rest = &rest[end + 2..];
    }
    out
}

/// ノート一覧
#[utoipa::path(
    get,
    path = "/api/notes",
    tag = "notes",
    params(ListNotesQuery),
    responses(
        (status = 200, body = Vec<note::Model>),
        (status = 400, description = "リクエストパラメータが不正", body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn list_notes(
    State(state): State<AppState>,
    JsonQuery(params): JsonQuery<ListNotesQuery>,
) -> Result<Json<Vec<note::Model>>, AppError> {
    let mut q = note::Entity::find().order_by_desc(note::Column::UpdatedAt);
    if let Some(sid) = params.strategy_id {
        q = q.filter(note::Column::StrategyId.eq(sid));
    }
    if let Some(status) = params.status.as_deref().filter(|s| !s.is_empty()) {
        q = q.filter(note::Column::Status.eq(status));
    }
    if let Some(tag) = params.type_tag.as_deref().filter(|s| !s.is_empty()) {
        q = q.filter(note::Column::TypeTag.eq(tag));
    }
    let items = q.all(&state.db).await?;
    Ok(Json(items))
}

/// ノート取得
#[utoipa::path(
    get,
    path = "/api/notes/{id}",
    tag = "notes",
    params(("id" = Uuid, Path, description = "ノート ID")),
    responses(
        (status = 200, body = note::Model),
        (status = 400, description = "リクエストパラメータが不正", body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn get_note(
    State(state): State<AppState>,
    JsonPath(id): JsonPath<Uuid>,
) -> Result<Json<note::Model>, AppError> {
    Ok(Json(find_note_or_404(&state.db, id).await?))
}

/// ノート作成
#[utoipa::path(
    post,
    path = "/api/notes",
    tag = "notes",
    request_body = CreateNoteRequest,
    responses(
        (status = 201, body = note::Model),
        (status = 400, description = "リクエストパラメータが不正", body = ErrorResponse),
        (status = 422, description = "リクエストボディのパースに失敗", body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn create_note(
    State(state): State<AppState>,
    JsonBody(payload): JsonBody<CreateNoteRequest>,
) -> Result<(StatusCode, Json<note::Model>), AppError> {
    let title = payload.title.trim().to_string();
    if title.is_empty() {
        return Err(AppError::Validation("title must not be empty".into()));
    }
    if let Some(fm) = payload.frontmatter_json.as_ref() {
        ensure_frontmatter_object(fm)?;
    }
    let status = payload.status.as_deref().unwrap_or("unread").to_string();
    if !ALLOWED_STATUSES.contains(&status.as_str()) {
        return Err(AppError::Validation(format!("invalid status: {status}")));
    }
    let created_by = payload
        .created_by_kind
        .as_deref()
        .unwrap_or("human")
        .to_string();
    if !ALLOWED_CREATED_BY.contains(&created_by.as_str()) {
        return Err(AppError::Validation(format!(
            "invalid created_by_kind: {created_by}"
        )));
    }

    let id = Uuid::new_v4();
    let txn = state.db.begin().await?;
    ensure_strategy_exists(&txn, payload.strategy_id).await?;

    let model = note::ActiveModel {
        id: Set(id),
        strategy_id: Set(payload.strategy_id),
        title: Set(title.clone()),
        body_md: Set(payload.body_md.clone()),
        frontmatter_json: Set(payload
            .frontmatter_json
            .clone()
            .unwrap_or_else(|| json!({}))),
        type_tag: Set(payload.type_tag.clone()),
        status: Set(status),
        trigger: Set(payload.trigger.map(|t| t.to_string())),
        trigger_label: Set(payload.trigger_label.clone()),
        created_by_kind: Set(created_by),
        created_at: NotSet,
        updated_at: NotSet,
    };
    let created = note::Entity::insert(model)
        .exec_with_returning(&txn)
        .await?;

    sync_note_refs(&txn, id, &payload.body_md).await?;

    change_history::record(
        &txn,
        TargetKind::Note,
        id,
        Op::Create,
        json!({ "title": title, "strategy_id": payload.strategy_id }),
        None,
    )
    .await?;

    txn.commit().await?;
    Ok((StatusCode::CREATED, Json(created)))
}

/// ノート更新
#[utoipa::path(
    patch,
    path = "/api/notes/{id}",
    tag = "notes",
    params(("id" = Uuid, Path, description = "ノート ID")),
    request_body = UpdateNoteRequest,
    responses(
        (status = 200, body = note::Model),
        (status = 400, description = "リクエストパラメータが不正", body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 422, description = "リクエストボディのパースに失敗", body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn update_note(
    State(state): State<AppState>,
    JsonPath(id): JsonPath<Uuid>,
    JsonBody(payload): JsonBody<UpdateNoteRequest>,
) -> Result<Json<note::Model>, AppError> {
    let current = find_note_or_404(&state.db, id).await?;
    let mut active = current.clone().into_active_model();
    let mut diff = serde_json::Map::new();
    let mut body_changed: Option<String> = None;

    if let Some(title) = payload.title {
        let title = title.trim().to_string();
        if title.is_empty() {
            return Err(AppError::Validation("title must not be empty".into()));
        }
        diff.insert(
            "title".into(),
            json!({ "from": current.title, "to": title }),
        );
        active.title = Set(title);
    }
    if let Some(body) = payload.body_md {
        diff.insert(
            "body_md".into(),
            json!({ "len_from": current.body_md.len(), "len_to": body.len() }),
        );
        active.body_md = Set(body.clone());
        body_changed = Some(body);
    }
    if let Some(fm) = payload.frontmatter_json {
        ensure_frontmatter_object(&fm)?;
        diff.insert(
            "frontmatter_json".into(),
            json!({ "from": current.frontmatter_json, "to": fm }),
        );
        active.frontmatter_json = Set(fm);
    }
    if let Some(tt) = payload.type_tag {
        diff.insert(
            "type_tag".into(),
            json!({ "from": current.type_tag, "to": tt }),
        );
        active.type_tag = Set(Some(tt));
    }
    if let Some(tr) = payload.trigger {
        let tr = tr.to_string();
        diff.insert(
            "trigger".into(),
            json!({ "from": current.trigger, "to": tr }),
        );
        active.trigger = Set(Some(tr));
    }
    if let Some(tl) = payload.trigger_label {
        diff.insert(
            "trigger_label".into(),
            json!({ "from": current.trigger_label, "to": tl }),
        );
        active.trigger_label = Set(Some(tl));
    }
    active.updated_at = Set(chrono::Utc::now().fixed_offset());

    let txn = state.db.begin().await?;
    let updated = active.update(&txn).await?;
    if let Some(body) = body_changed.as_deref() {
        sync_note_refs(&txn, id, body).await?;
    }
    if !diff.is_empty() {
        change_history::record(
            &txn,
            TargetKind::Note,
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

/// ノート削除
#[utoipa::path(
    delete,
    path = "/api/notes/{id}",
    tag = "notes",
    params(("id" = Uuid, Path, description = "ノート ID")),
    responses(
        (status = 204),
        (status = 400, description = "リクエストパラメータが不正", body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn delete_note(
    State(state): State<AppState>,
    JsonPath(id): JsonPath<Uuid>,
) -> Result<StatusCode, AppError> {
    let txn = state.db.begin().await?;
    let res = note::Entity::delete_by_id(id).exec(&txn).await?;
    if res.rows_affected == 0 {
        return Err(AppError::NotFound(format!("note {id} not found")));
    }
    change_history::record(&txn, TargetKind::Note, id, Op::Delete, json!({}), None).await?;
    txn.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn change_note_status_from(
    state: &AppState,
    current: note::Model,
    new_status: &str,
    label: Option<String>,
) -> Result<note::Model, AppError> {
    let id = current.id;
    let mut active = current.clone().into_active_model();
    active.status = Set(new_status.to_string());
    active.updated_at = Set(chrono::Utc::now().fixed_offset());
    let txn = state.db.begin().await?;
    let updated = active.update(&txn).await?;
    change_history::record(
        &txn,
        TargetKind::Note,
        id,
        Op::StatusChange,
        json!({ "from": current.status, "to": new_status, "label": label }),
        label,
    )
    .await?;
    txn.commit().await?;
    Ok(updated)
}

async fn change_note_status(
    state: &AppState,
    id: Uuid,
    new_status: &str,
    label: Option<String>,
) -> Result<note::Model, AppError> {
    let current = find_note_or_404(&state.db, id).await?;
    change_note_status_from(state, current, new_status, label).await
}

/// ノートを approved に遷移
#[utoipa::path(
    post,
    path = "/api/notes/{id}/approve",
    tag = "notes",
    params(("id" = Uuid, Path, description = "ノート ID")),
    request_body = ChangeStatusRequest,
    responses(
        (status = 200, body = note::Model),
        (status = 400, description = "リクエストパラメータが不正", body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 422, description = "リクエストボディのパースに失敗", body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn approve_note(
    State(state): State<AppState>,
    JsonPath(id): JsonPath<Uuid>,
    JsonBody(payload): JsonBody<ChangeStatusRequest>,
) -> Result<Json<note::Model>, AppError> {
    Ok(Json(
        change_note_status(&state, id, "approved", payload.label).await?,
    ))
}

/// ノートを rejected に遷移
#[utoipa::path(
    post,
    path = "/api/notes/{id}/reject",
    tag = "notes",
    params(("id" = Uuid, Path, description = "ノート ID")),
    request_body = ChangeStatusRequest,
    responses(
        (status = 200, body = note::Model),
        (status = 400, description = "リクエストパラメータが不正", body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 422, description = "リクエストボディのパースに失敗", body = ErrorResponse),
        (status = 500, body = ErrorResponse),
        (status = 503, description = "agent task client が未設定", body = ErrorResponse),
    )
)]
pub async fn reject_note(
    State(state): State<AppState>,
    JsonPath(id): JsonPath<Uuid>,
    JsonBody(payload): JsonBody<ChangeStatusRequest>,
) -> Result<Json<note::Model>, AppError> {
    let current = find_note_or_404(&state.db, id).await?;
    // 却下確定前の check-then-act。ほぼ同時に reject が 2 回届くと両方通過し得るが、
    // frontend は mutation pending 中ボタンを disable するため実運用では起きない。
    if current.status == "rejected" {
        return Ok(Json(current));
    }

    let prompt = format!(
        "ノート「{}」(id: {}) がレビューで却下されました。付いているコメントを確認し、指摘を反映してください。",
        current.title, current.id
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
        change_note_status_from(&state, current, "rejected", payload.label).await?,
    ))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use axum_test::TestServer;
    use rstest::rstest;
    use serde_json::Value;
    use sqlx::PgPool;

    use super::*;
    use crate::agent_client::{AgentTaskError, FakeAgentTaskClient, SharedAgentTaskClient};
    use crate::entities::sea_orm_active_enums::StrategyTaskPhase;
    use crate::entities::strategy_task;
    use crate::testing::{create_test_server_with_db_and_agent_client, insert_test_strategy};

    #[rstest]
    #[case::single("hello [[stock:7203]]", vec![("stock", "7203")])]
    #[case::multiple("a [[indicator:USDJPY]] b [[theme:weak-jpy]]", vec![("indicator", "USDJPY"), ("theme", "weak-jpy")])]
    #[case::unknown_kind_ignored("[[foo:bar]] [[stock:9984]]", vec![("stock", "9984")])]
    #[case::no_prefix_ignored("[[7203]]", vec![])]
    #[case::empty("", vec![])]
    fn test_extract_refs(#[case] body: &str, #[case] expected: Vec<(&str, &str)>) {
        let got = extract_refs(body);
        let got: Vec<(&str, &str)> = got.iter().map(|(k, i)| (k.as_str(), i.as_str())).collect();
        assert_eq!(got, expected);
    }

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

    async fn create_test_note(server: &TestServer, strategy_id: Uuid, title: &str) -> Uuid {
        let res = server
            .post("/api/notes")
            .json(&json!({
                "strategy_id": strategy_id,
                "title": title,
                "body_md": "body",
            }))
            .await;
        res.assert_status(StatusCode::CREATED);
        let body: Value = res.json();
        Uuid::parse_str(body["id"].as_str().expect("id")).expect("uuid")
    }

    #[sqlx::test(migrations = false)]
    async fn reject_note_submits_single_review_task_referencing_note(pool: PgPool) {
        let fake = Arc::new(FakeAgentTaskClient::new());
        let agent_client: SharedAgentTaskClient = fake.clone();
        let (db, server) = create_test_server_with_db_and_agent_client(pool, agent_client).await;
        let strategy_id = insert_test_strategy(&db, "s").await;
        let note_id = create_test_note(&server, strategy_id, "タイトル").await;

        let res = server
            .post(&format!("/api/notes/{note_id}/reject"))
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
                "id": note_id,
                "strategy_id": strategy_id,
                "title": "タイトル",
                "body_md": "body",
                "frontmatter_json": {},
                "type_tag": null,
                "status": "rejected",
                "trigger": null,
                "trigger_label": null,
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
                    "ノート「タイトル」(id: {note_id}) がレビューで却下されました。\
付いているコメントを確認し、指摘を反映してください。"
                ),
                phase: StrategyTaskPhase::Running,
            }],
        );
    }

    #[sqlx::test(migrations = false)]
    async fn rejecting_already_rejected_note_does_not_resubmit(pool: PgPool) {
        let fake = Arc::new(FakeAgentTaskClient::new());
        let agent_client: SharedAgentTaskClient = fake.clone();
        let (db, server) = create_test_server_with_db_and_agent_client(pool, agent_client).await;
        let strategy_id = insert_test_strategy(&db, "s").await;
        let note_id = create_test_note(&server, strategy_id, "t").await;

        for _ in 0..2 {
            let res = server
                .post(&format!("/api/notes/{note_id}/reject"))
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
    async fn reject_note_leaves_status_unchanged_when_agent_submission_fails(pool: PgPool) {
        let fake = Arc::new(FakeAgentTaskClient::new());
        fake.set_submit_error(AgentTaskError::NotConfigured).await;
        let agent_client: SharedAgentTaskClient = fake;
        let (db, server) = create_test_server_with_db_and_agent_client(pool, agent_client).await;
        let strategy_id = insert_test_strategy(&db, "s").await;
        let note_id = create_test_note(&server, strategy_id, "t").await;

        let res = server
            .post(&format!("/api/notes/{note_id}/reject"))
            .json(&json!({}))
            .await;
        res.assert_status(StatusCode::SERVICE_UNAVAILABLE);

        let note = find_note_or_404(&db, note_id).await.unwrap();
        assert_eq!(note.status, "unread");
    }
}
