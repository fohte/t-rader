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
use crate::entities::{note, note_version};
use crate::error::{AppError, ErrorResponse};
use crate::extractors::{JsonBody, JsonPath, JsonQuery};
use crate::models::{CreateNoteRequest, NoteResponse, UpdateNoteRequest};
use crate::services::change_history::{self, Actor, Op, TargetKind};
use crate::services::note_versions::{
    self, AppendVersion, current_note_ids_with_status, find_current_versions,
    find_initial_created_by_kind,
};
use crate::services::strategies::ensure_strategy_exists;

pub(crate) mod status;
pub(crate) use status::{__path_approve_note, __path_reject_note};
pub use status::{approve_note, reject_note};

const ALLOWED_STATUSES: [&str; 3] = ["approved", "unread", "rejected"];
const ALLOWED_CREATED_BY: [&str; 2] = ["human", "llm"];

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

pub(crate) async fn find_note_or_404(
    db: &sea_orm::DatabaseConnection,
    id: Uuid,
) -> Result<note::Model, AppError> {
    note::Entity::find_by_id(id)
        .one(db)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("note {id} not found")))
}

struct CurrentNote {
    note: note::Model,
    version: note_version::Model,
    created_by_kind: String,
}

async fn find_current_note_or_404<C: sea_orm::ConnectionTrait>(
    db: &C,
    id: Uuid,
) -> Result<CurrentNote, AppError> {
    let note = note::Entity::find_by_id(id)
        .one(db)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("note {id} not found")))?;
    let version = note_versions::find_current_version(db, id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("current version for note {id} not found")))?;
    let created_by_kind = find_initial_created_by_kind(db, &[id])
        .await?
        .remove(&id)
        .ok_or_else(|| AppError::NotFound(format!("initial version for note {id} not found")))?;
    Ok(CurrentNote {
        note,
        version,
        created_by_kind,
    })
}

fn current_note_response(current: CurrentNote) -> NoteResponse {
    NoteResponse::from_current_version(current.note, current.version, current.created_by_kind)
}

/// ノート一覧
#[utoipa::path(
    get,
    path = "/api/notes",
    tag = "notes",
    params(ListNotesQuery),
    responses(
        (status = 200, body = Vec<NoteResponse>),
        (status = 400, description = "リクエストパラメータが不正", body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn list_notes(
    State(state): State<AppState>,
    JsonQuery(params): JsonQuery<ListNotesQuery>,
) -> Result<Json<Vec<NoteResponse>>, AppError> {
    let mut q = note::Entity::find().order_by_desc(note::Column::UpdatedAt);
    if let Some(sid) = params.strategy_id {
        q = q.filter(note::Column::StrategyId.eq(sid));
    }
    if let Some(status) = params.status.as_deref().filter(|s| !s.is_empty()) {
        q = q.filter(note::Column::Id.in_subquery(current_note_ids_with_status(status)));
    }
    if let Some(tag) = params.type_tag.as_deref().filter(|s| !s.is_empty()) {
        q = q.filter(note::Column::TypeTag.eq(tag));
    }
    let items = q.all(&state.db).await?;
    let versions = find_current_versions(
        &state.db,
        &items.iter().map(|item| item.id).collect::<Vec<_>>(),
    )
    .await?;
    let creators = find_initial_created_by_kind(
        &state.db,
        &items.iter().map(|item| item.id).collect::<Vec<_>>(),
    )
    .await?;
    let responses = items
        .into_iter()
        .map(|item| {
            let version = versions.get(&item.id).cloned().ok_or_else(|| {
                AppError::NotFound(format!("current version for note {} not found", item.id))
            })?;
            let created_by_kind = creators.get(&item.id).cloned().ok_or_else(|| {
                AppError::NotFound(format!("initial version for note {} not found", item.id))
            })?;
            Ok(NoteResponse::from_current_version(
                item,
                version,
                created_by_kind,
            ))
        })
        .collect::<Result<Vec<_>, AppError>>()?;
    Ok(Json(responses))
}

/// ノート取得
#[utoipa::path(
    get,
    path = "/api/notes/{id}",
    tag = "notes",
    params(("id" = Uuid, Path, description = "ノート ID")),
    responses(
        (status = 200, body = NoteResponse),
        (status = 400, description = "リクエストパラメータが不正", body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn get_note(
    State(state): State<AppState>,
    JsonPath(id): JsonPath<Uuid>,
) -> Result<Json<NoteResponse>, AppError> {
    Ok(Json(current_note_response(
        find_current_note_or_404(&state.db, id).await?,
    )))
}

/// ノート作成
#[utoipa::path(
    post,
    path = "/api/notes",
    tag = "notes",
    request_body = CreateNoteRequest,
    responses(
        (status = 201, body = NoteResponse),
        (status = 400, description = "リクエストパラメータが不正", body = ErrorResponse),
        (status = 415, description = "Content-Type ヘッダが application/json ではない", body = ErrorResponse),
        (status = 422, description = "リクエストボディのパースに失敗", body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn create_note(
    State(state): State<AppState>,
    JsonBody(payload): JsonBody<CreateNoteRequest>,
) -> Result<(StatusCode, Json<NoteResponse>), AppError> {
    let title = payload.title.trim().to_string();
    if title.is_empty() {
        return Err(AppError::Validation("title must not be empty".into()));
    }
    if let Some(fm) = payload.frontmatter_json.as_ref() {
        ensure_frontmatter_object(fm)?;
    }
    let status = payload
        .status
        .as_deref()
        .unwrap_or(note_versions::INITIAL_NOTE_STATUS)
        .to_string();
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
    let history_title = title.clone();
    let txn = state.db.begin().await?;
    if let Some(strategy_id) = payload.strategy_id {
        ensure_strategy_exists(&txn, strategy_id).await?;
    }

    let model = note::ActiveModel {
        id: Set(id),
        strategy_id: Set(payload.strategy_id),
        type_tag: Set(payload.type_tag.clone()),
        trigger: Set(payload.trigger.map(|t| t.to_string())),
        trigger_label: Set(payload.trigger_label.clone()),
        created_at: NotSet,
        updated_at: NotSet,
        execution_id: Set(None),
    };
    note::Entity::insert(model)
        .exec_with_returning(&txn)
        .await?;

    let mut version = note_versions::append_version(
        &txn,
        id,
        AppendVersion {
            title,
            body_md: payload.body_md,
            frontmatter_json: payload.frontmatter_json.unwrap_or_else(|| json!({})),
            graphs_json: json!([]),
            created_by_kind: created_by.clone(),
            execution_id: None,
            change_reason: None,
            change_diff: Some(json!({
                "title": history_title,
                "strategy_id": payload.strategy_id,
            })),
            actor: Actor::Human,
        },
    )
    .await?;
    if status != note_versions::INITIAL_NOTE_STATUS {
        let reviewed_at = chrono::Utc::now().fixed_offset();
        version = note_version::ActiveModel {
            id: Set(version.id),
            status: Set(status),
            reviewed_at: Set(Some(reviewed_at)),
            ..Default::default()
        }
        .update(&txn)
        .await?;
    }
    let created = note::Entity::find_by_id(id)
        .one(&txn)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("note {id} not found")))?;

    txn.commit().await?;
    Ok((
        StatusCode::CREATED,
        Json(NoteResponse::from_current_version(
            created, version, created_by,
        )),
    ))
}

/// ノート更新
#[utoipa::path(
    patch,
    path = "/api/notes/{id}",
    tag = "notes",
    params(("id" = Uuid, Path, description = "ノート ID")),
    request_body = UpdateNoteRequest,
    responses(
        (status = 200, body = NoteResponse),
        (status = 400, description = "リクエストパラメータが不正", body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 415, description = "Content-Type ヘッダが application/json ではない", body = ErrorResponse),
        (status = 422, description = "リクエストボディのパースに失敗", body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn update_note(
    State(state): State<AppState>,
    JsonPath(id): JsonPath<Uuid>,
    JsonBody(payload): JsonBody<UpdateNoteRequest>,
) -> Result<Json<NoteResponse>, AppError> {
    let current = find_current_note_or_404(&state.db, id).await?;
    let current_note = current.note.clone();
    let current_version = current.version.clone();
    let mut active = current_note.clone().into_active_model();
    let mut diff = serde_json::Map::new();
    let mut new_title = current_version.title.clone();
    let mut body_md = current_version.body_md.clone();
    let mut frontmatter_json = current_version.frontmatter_json.clone();
    let mut version_changed = false;

    if let Some(title) = payload.title {
        let updated_title = title.trim().to_string();
        if updated_title.is_empty() {
            return Err(AppError::Validation("title must not be empty".into()));
        }
        diff.insert(
            "title".into(),
            json!({ "from": current_version.title, "to": updated_title }),
        );
        version_changed = true;
        new_title = updated_title;
    }
    if let Some(body) = payload.body_md {
        diff.insert(
            "body_md".into(),
            json!({ "len_from": current_version.body_md.len(), "len_to": body.len() }),
        );
        body_md = body;
        version_changed = true;
    }
    if let Some(fm) = payload.frontmatter_json {
        ensure_frontmatter_object(&fm)?;
        diff.insert(
            "frontmatter_json".into(),
            json!({ "from": current_version.frontmatter_json, "to": fm }),
        );
        frontmatter_json = fm;
        version_changed = true;
    }
    if let Some(tt) = payload.type_tag {
        diff.insert(
            "type_tag".into(),
            json!({ "from": current_note.type_tag, "to": tt }),
        );
        active.type_tag = Set(Some(tt));
    }
    if let Some(tr) = payload.trigger {
        let tr = tr.to_string();
        diff.insert(
            "trigger".into(),
            json!({ "from": current_note.trigger, "to": tr }),
        );
        active.trigger = Set(Some(tr));
    }
    if let Some(tl) = payload.trigger_label {
        diff.insert(
            "trigger_label".into(),
            json!({ "from": current_note.trigger_label, "to": tl }),
        );
        active.trigger_label = Set(Some(tl));
    }

    let txn = state.db.begin().await?;
    active.updated_at = if version_changed {
        NotSet
    } else {
        Set(chrono::Utc::now().fixed_offset())
    };
    active.update(&txn).await?;
    if version_changed {
        note_versions::append_version(
            &txn,
            id,
            AppendVersion {
                title: new_title,
                body_md,
                frontmatter_json,
                graphs_json: current_version.graphs_json,
                created_by_kind: "human".into(),
                execution_id: None,
                change_reason: None,
                change_diff: Some(serde_json::Value::Object(diff)),
                actor: Actor::Human,
            },
        )
        .await?;
    } else if !diff.is_empty() {
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
    let updated_current = find_current_note_or_404(&txn, id).await?;
    txn.commit().await?;

    Ok(Json(current_note_response(updated_current)))
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

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use axum_test::TestServer;
    use serde_json::Value;
    use sqlx::PgPool;

    use super::*;
    use crate::agent_client::{AgentTaskError, FakeAgentTaskClient, SharedAgentTaskClient};
    use crate::entities::comment;
    use crate::entities::sea_orm_active_enums::StrategyTaskPhase;
    use crate::entities::strategy_task;
    use crate::services::agent_config;
    use crate::services::strategy_tasks::DEFAULT_PURPOSE;
    use crate::testing::{
        create_test_server_with_db, create_test_server_with_db_and_agent_client,
        insert_test_strategy,
    };

    const INVALID_NOTE_BODY: &str = "[[bogus:one]] [[bare-demo]]";
    const INVALID_NOTE_TOKEN_ERROR: &str = concat!(
        "ノートのトークンに問題があります:\n",
        "- 本文のトークン \"[[bogus:one]]\": 未知の prefix `bogus` です\n",
        "- 本文のトークン \"[[bare-demo]]\": kind:id の形式で prefix を指定してください\n",
        "許可される形式: `[[stock:<id>]]`, `[[indicator:<id>]]`, `[[sector:<id>]]`, `[[theme:<id>]]`, `[[anno:<id>]]`。`[[graph:<id>]]` は graphs[].id に存在し、空行区切りブロック内で単独にしてください。graphs[].nodes[].ref では参照 4 種のみ使用できます。",
    );

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
        create_test_note_with_body(server, strategy_id, title, "body").await
    }

    async fn create_test_note_with_body(
        server: &TestServer,
        strategy_id: Uuid,
        title: &str,
        body_md: &str,
    ) -> Uuid {
        let res = server
            .post("/api/notes")
            .json(&json!({
                "strategy_id": strategy_id,
                "title": title,
                "body_md": body_md,
            }))
            .await;
        res.assert_status(StatusCode::CREATED);
        let body: Value = res.json();
        Uuid::parse_str(body["id"].as_str().expect("id")).expect("uuid")
    }

    /// strategy を持たないノートは execution (戦略タスク実行) に紐づき得ない、という
    /// note_strategy_id_execution_id_check CHECK 制約の回帰テスト。
    #[sqlx::test(migrations = false)]
    async fn note_without_strategy_id_rejects_execution_id(pool: PgPool) {
        let (db, _server) = create_test_server_with_db(pool).await;

        let result = note::ActiveModel {
            id: Set(Uuid::new_v4()),
            strategy_id: Set(None),
            type_tag: Set(None),
            trigger: Set(None),
            trigger_label: Set(None),
            created_at: NotSet,
            updated_at: NotSet,
            execution_id: Set(Some("exec-1".into())),
        }
        .insert(&db)
        .await;

        assert!(result.is_err());
    }

    #[sqlx::test(migrations = false)]
    async fn create_note_without_strategy_id_succeeds(pool: PgPool) {
        let (_db, server) = create_test_server_with_db(pool).await;

        let res = server
            .post("/api/notes")
            .json(&json!({
                "title": "市況ノート",
                "body_md": "body",
            }))
            .await;
        res.assert_status(StatusCode::CREATED);
        let mut body: Value = res.json();
        let obj = body.as_object_mut().unwrap();
        obj.remove("id");
        obj.remove("created_at");
        obj.remove("updated_at");
        assert_eq!(
            body,
            json!({
                "strategy_id": null,
                "title": "市況ノート",
                "body_md": "body",
                "frontmatter_json": {},
                "graphs_json": [],
                "type_tag": null,
                "status": "unread",
                "trigger": null,
                "trigger_label": null,
                "created_by_kind": "human",
                "execution_id": null,
            }),
        );
    }

    #[sqlx::test(migrations = false)]
    async fn create_note_rejects_invalid_tokens_without_saving(pool: PgPool) {
        let (db, server) = create_test_server_with_db(pool).await;

        let res = server
            .post("/api/notes")
            .json(&json!({
                "title": "token validation",
                "body_md": INVALID_NOTE_BODY,
            }))
            .await;
        let response = res.json::<Value>();
        let saved_notes = note::Entity::find().all(&db).await.unwrap();

        assert_eq!(
            (res.status_code(), response, saved_notes.len()),
            (
                StatusCode::BAD_REQUEST,
                json!({"error": INVALID_NOTE_TOKEN_ERROR}),
                0,
            ),
        );
    }

    #[sqlx::test(migrations = false)]
    async fn update_note_rejects_invalid_tokens_and_keeps_original_body(pool: PgPool) {
        let (db, server) = create_test_server_with_db(pool).await;
        let strategy_id = insert_test_strategy(&db, "strategy").await;
        let note_id = create_test_note_with_body(&server, strategy_id, "title", "original").await;

        let res = server
            .patch(&format!("/api/notes/{note_id}"))
            .json(&json!({"body_md": INVALID_NOTE_BODY}))
            .await;
        let response = res.json::<Value>();
        let saved_body = note_versions::find_current_version(&db, note_id)
            .await
            .unwrap()
            .map(|version| version.body_md);

        assert_eq!(
            (res.status_code(), response, saved_body),
            (
                StatusCode::BAD_REQUEST,
                json!({"error": INVALID_NOTE_TOKEN_ERROR}),
                Some("original".to_string()),
            ),
        );
    }

    #[sqlx::test(migrations = false)]
    async fn reject_note_without_strategy_id_does_not_submit_task(pool: PgPool) {
        let fake = Arc::new(FakeAgentTaskClient::new());
        let agent_client: SharedAgentTaskClient = fake.clone();
        let (db, server) = create_test_server_with_db_and_agent_client(pool, agent_client).await;

        let res = server
            .post("/api/notes")
            .json(&json!({
                "title": "市況ノート",
                "body_md": "body",
            }))
            .await;
        res.assert_status(StatusCode::CREATED);
        let note_id =
            Uuid::parse_str(res.json::<Value>()["id"].as_str().expect("id")).expect("uuid");

        let res = server
            .post(&format!("/api/notes/{note_id}/reject"))
            .json(&json!({}))
            .await;
        res.assert_status_ok();
        let mut body: Value = res.json();
        let obj = body.as_object_mut().unwrap();
        obj.remove("id");
        obj.remove("created_at");
        obj.remove("updated_at");
        assert_eq!(
            body,
            json!({
                "strategy_id": null,
                "title": "市況ノート",
                "body_md": "body",
                "frontmatter_json": {},
                "graphs_json": [],
                "type_tag": null,
                "status": "rejected",
                "trigger": null,
                "trigger_label": null,
                "created_by_kind": "human",
                "execution_id": null,
            }),
        );

        let tasks = strategy_task::Entity::find().all(&db).await.unwrap();
        assert_eq!(tasks, vec![]);
    }

    #[sqlx::test(migrations = false)]
    async fn reject_note_submits_single_review_task_referencing_note(pool: PgPool) {
        let fake = Arc::new(FakeAgentTaskClient::new());
        let agent_client: SharedAgentTaskClient = fake.clone();
        let (db, server) = create_test_server_with_db_and_agent_client(pool, agent_client).await;
        let strategy_id = insert_test_strategy(&db, "s").await;
        agent_config::create(&db, DEFAULT_PURPOSE.to_string())
            .await
            .expect("insert test agent_config");
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
                "graphs_json": [],
                "type_tag": null,
                "status": "rejected",
                "trigger": null,
                "trigger_label": null,
                "created_by_kind": "human",
                "execution_id": null,
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
        agent_config::create(&db, DEFAULT_PURPOSE.to_string())
            .await
            .expect("insert test agent_config");
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
        agent_config::create(&db, DEFAULT_PURPOSE.to_string())
            .await
            .expect("insert test agent_config");
        let note_id = create_test_note(&server, strategy_id, "t").await;

        let res = server
            .post(&format!("/api/notes/{note_id}/reject"))
            .json(&json!({}))
            .await;
        res.assert_status(StatusCode::SERVICE_UNAVAILABLE);

        let current_version = note_versions::find_current_version(&db, note_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(current_version.status, "unread");
    }

    #[sqlx::test(migrations = false)]
    async fn update_note_reanchors_comment_when_body_md_changes(pool: PgPool) {
        let (db, server) = create_test_server_with_db(pool).await;
        let strategy_id = insert_test_strategy(&db, "s").await;
        let note_id = create_test_note_with_body(
            &server,
            strategy_id,
            "note",
            indoc::indoc! {"
                line one
                line two
                line three"},
        )
        .await;

        let created_comment = server
            .post("/api/comments")
            .json(&json!({
                "target_kind": "note",
                "target_id": note_id,
                "body": "fix this",
                "anchor_text": "line two",
            }))
            .await;
        created_comment.assert_status(StatusCode::CREATED);
        let comment_id =
            Uuid::parse_str(created_comment.json::<Value>()["id"].as_str().expect("id"))
                .expect("uuid");

        let res = server
            .patch(&format!("/api/notes/{note_id}"))
            .json(&json!({
                "body_md": indoc::indoc! {"
                    prefix
                    line one
                    line two
                    line three"},
            }))
            .await;
        res.assert_status_ok();

        let mut updated_comment = comment::Entity::find_by_id(comment_id)
            .one(&db)
            .await
            .expect("query")
            .expect("comment exists");
        updated_comment.created_at = chrono::DateTime::<chrono::Utc>::UNIX_EPOCH.fixed_offset();
        assert_eq!(
            updated_comment,
            comment::Model {
                id: comment_id,
                target_kind: "note".into(),
                target_id: note_id,
                parent_id: None,
                body: "fix this".into(),
                author_kind: "human".into(),
                author_label: "user".into(),
                created_at: chrono::DateTime::<chrono::Utc>::UNIX_EPOCH.fixed_offset(),
                resolved: false,
                anchor_text: Some("line two".into()),
                start_line: Some(3),
                end_line: Some(3),
                drifted: false,
            },
        );
    }
}
