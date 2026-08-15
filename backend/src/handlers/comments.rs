use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use sea_orm::ActiveValue::{NotSet, Set};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, IntoActiveModel, QueryFilter, QueryOrder,
    TransactionTrait,
};
use serde::Deserialize;
use serde_json::json;
use utoipa::IntoParams;
use uuid::Uuid;

use crate::AppState;
use crate::entities::comment;
use crate::error::{AppError, ErrorResponse};
use crate::extractors::{JsonBody, JsonPath, JsonQuery};
use crate::models::{CreateCommentRequest, UpdateCommentRequest};
use crate::services::change_history::{self, Op, TargetKind};
use crate::services::comment_anchor;

const ALLOWED_TARGET_KIND: [&str; 2] = ["note", "annotation"];
const ALLOWED_AUTHOR_KIND: [&str; 2] = ["human", "llm"];

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ListCommentsQuery {
    pub target_kind: String,
    pub target_id: Uuid,
}

/// コメント一覧。`target_kind` + `target_id` でフィルタ。スレッドは parent_id で表現する。
#[utoipa::path(
    get,
    path = "/api/comments",
    tag = "comments",
    params(ListCommentsQuery),
    responses(
        (status = 200, body = Vec<comment::Model>),
        (status = 400, description = "リクエストパラメータが不正", body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn list_comments(
    State(state): State<AppState>,
    JsonQuery(p): JsonQuery<ListCommentsQuery>,
) -> Result<Json<Vec<comment::Model>>, AppError> {
    if !ALLOWED_TARGET_KIND.contains(&p.target_kind.as_str()) {
        return Err(AppError::Validation(format!(
            "invalid target_kind: {}",
            p.target_kind
        )));
    }
    let items = comment::Entity::find()
        .filter(comment::Column::TargetKind.eq(p.target_kind))
        .filter(comment::Column::TargetId.eq(p.target_id))
        .order_by_asc(comment::Column::CreatedAt)
        .all(&state.db)
        .await?;
    Ok(Json(items))
}

/// コメント投稿
#[utoipa::path(
    post,
    path = "/api/comments",
    tag = "comments",
    request_body = CreateCommentRequest,
    responses(
        (status = 201, body = comment::Model),
        (status = 400, description = "リクエストパラメータが不正", body = ErrorResponse),
        (status = 422, description = "リクエストボディのパースに失敗", body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn create_comment(
    State(state): State<AppState>,
    JsonBody(p): JsonBody<CreateCommentRequest>,
) -> Result<(StatusCode, Json<comment::Model>), AppError> {
    if !ALLOWED_TARGET_KIND.contains(&p.target_kind.as_str()) {
        return Err(AppError::Validation(format!(
            "invalid target_kind: {}",
            p.target_kind
        )));
    }
    if p.body.trim().is_empty() {
        return Err(AppError::Validation("body must not be empty".into()));
    }
    let author_kind = p.author_kind.as_deref().unwrap_or("human").to_string();
    if !ALLOWED_AUTHOR_KIND.contains(&author_kind.as_str()) {
        return Err(AppError::Validation(format!(
            "invalid author_kind: {author_kind}"
        )));
    }
    let author_label = p.author_label.as_deref().unwrap_or("user").to_string();

    // 親コメント指定時は同一 target に属することを確認する (誤った target をまたぐ
    // 返信スレッドが list_comments で迷子表示されるのを防ぐ)
    if let Some(parent_id) = p.parent_id {
        let parent = comment::Entity::find_by_id(parent_id)
            .one(&state.db)
            .await?
            .ok_or_else(|| AppError::Validation(format!("parent comment {parent_id} not found")))?;
        if parent.target_kind != p.target_kind || parent.target_id != p.target_id {
            return Err(AppError::Validation(
                "parent comment belongs to a different target".into(),
            ));
        }
    }

    let anchor_text = p
        .anchor_text
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let (anchor_text, start_line, end_line, drifted) = match anchor_text {
        None => (None, None, None, false),
        Some(anchor_text) => {
            let anchor_text = anchor_text.to_string();
            let (start_line, end_line, drifted) = comment_anchor::resolve_new_anchor(
                &state.db,
                &p.target_kind,
                p.target_id,
                &anchor_text,
            )
            .await?;
            (Some(anchor_text), start_line, end_line, drifted)
        }
    };

    let id = Uuid::new_v4();
    let model = comment::ActiveModel {
        id: Set(id),
        target_kind: Set(p.target_kind.clone()),
        target_id: Set(p.target_id),
        parent_id: Set(p.parent_id),
        body: Set(p.body.clone()),
        author_kind: Set(author_kind),
        author_label: Set(author_label),
        resolved: NotSet,
        created_at: NotSet,
        anchor_text: Set(anchor_text),
        start_line: Set(start_line),
        end_line: Set(end_line),
        drifted: Set(drifted),
    };
    let txn = state.db.begin().await?;
    let created = comment::Entity::insert(model)
        .exec_with_returning(&txn)
        .await?;
    change_history::record(
        &txn,
        TargetKind::Comment,
        id,
        Op::Create,
        json!({ "target_kind": p.target_kind, "target_id": p.target_id, "parent_id": p.parent_id }),
        None,
    )
    .await?;
    txn.commit().await?;

    Ok((StatusCode::CREATED, Json(created)))
}

/// コメントの resolved を更新する
#[utoipa::path(
    patch,
    path = "/api/comments/{id}",
    tag = "comments",
    params(("id" = Uuid, Path, description = "コメント ID")),
    request_body = UpdateCommentRequest,
    responses(
        (status = 200, body = comment::Model),
        (status = 400, description = "リクエストパラメータが不正", body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 422, description = "リクエストボディのパースに失敗", body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn update_comment(
    State(state): State<AppState>,
    JsonPath(id): JsonPath<Uuid>,
    JsonBody(payload): JsonBody<UpdateCommentRequest>,
) -> Result<Json<comment::Model>, AppError> {
    let current = comment::Entity::find_by_id(id)
        .one(&state.db)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("comment {id} not found")))?;
    if current.resolved == payload.resolved {
        return Ok(Json(current));
    }
    let from = current.resolved;
    let mut active = current.into_active_model();
    active.resolved = Set(payload.resolved);

    let txn = state.db.begin().await?;
    let updated = active.update(&txn).await?;
    change_history::record(
        &txn,
        TargetKind::Comment,
        id,
        Op::StatusChange,
        json!({ "from": from, "to": payload.resolved }),
        None,
    )
    .await?;
    txn.commit().await?;

    Ok(Json(updated))
}

/// コメント削除
#[utoipa::path(
    delete,
    path = "/api/comments/{id}",
    tag = "comments",
    params(("id" = Uuid, Path, description = "コメント ID")),
    responses(
        (status = 204),
        (status = 400, description = "リクエストパラメータが不正", body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn delete_comment(
    State(state): State<AppState>,
    JsonPath(id): JsonPath<Uuid>,
) -> Result<StatusCode, AppError> {
    let txn = state.db.begin().await?;
    let res = comment::Entity::delete_by_id(id).exec(&txn).await?;
    if res.rows_affected == 0 {
        return Err(AppError::NotFound(format!("comment {id} not found")));
    }
    change_history::record(&txn, TargetKind::Comment, id, Op::Delete, json!({}), None).await?;
    txn.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;
    use serde_json::{Value, json};
    use sqlx::PgPool;

    use crate::testing::create_test_server;

    fn normalize(mut value: Value) -> Value {
        for key in ["id", "target_id", "created_at"] {
            if let Some(v) = value.get_mut(key) {
                *v = Value::String(format!("<{key}>"));
            }
        }
        value
    }

    async fn create_note_comment(server: &axum_test::TestServer) -> Value {
        server
            .post("/api/comments")
            .json(&json!({
                "target_kind": "note",
                "target_id": uuid::Uuid::new_v4(),
                "body": "fix this",
            }))
            .await
            .json()
    }

    #[sqlx::test(migrations = false)]
    async fn update_comment_sets_resolved(pool: PgPool) {
        let server = create_test_server(pool).await;
        let created = create_note_comment(&server).await;
        let id = created["id"].as_str().expect("id");

        let res = server
            .patch(&format!("/api/comments/{id}"))
            .json(&json!({ "resolved": true }))
            .await;
        res.assert_status_ok();
        assert_eq!(
            normalize(res.json()),
            json!({
                "id": "<id>",
                "target_kind": "note",
                "target_id": "<target_id>",
                "parent_id": null,
                "body": "fix this",
                "author_kind": "human",
                "author_label": "user",
                "resolved": true,
                "created_at": "<created_at>",
                "anchor_text": null,
                "start_line": null,
                "end_line": null,
                "drifted": false,
            }),
        );

        let res = server
            .patch(&format!("/api/comments/{id}"))
            .json(&json!({ "resolved": false }))
            .await;
        res.assert_status_ok();
        assert_eq!(
            normalize(res.json()),
            json!({
                "id": "<id>",
                "target_kind": "note",
                "target_id": "<target_id>",
                "parent_id": null,
                "body": "fix this",
                "author_kind": "human",
                "author_label": "user",
                "resolved": false,
                "created_at": "<created_at>",
                "anchor_text": null,
                "start_line": null,
                "end_line": null,
                "drifted": false,
            }),
        );
    }

    #[sqlx::test(migrations = false)]
    async fn update_comment_missing_id_is_404(pool: PgPool) {
        let server = create_test_server(pool).await;
        let res = server
            .patch(&format!("/api/comments/{}", uuid::Uuid::new_v4()))
            .json(&json!({ "resolved": true }))
            .await;
        res.assert_status(StatusCode::NOT_FOUND);
    }

    async fn create_note(
        server: &axum_test::TestServer,
        strategy_id: &str,
        body_md: &str,
    ) -> String {
        let created = server
            .post("/api/notes")
            .json(&json!({
                "strategy_id": strategy_id,
                "title": "note",
                "body_md": body_md,
            }))
            .await;
        created.assert_status(StatusCode::CREATED);
        created.json::<Value>()["id"]
            .as_str()
            .expect("id")
            .to_string()
    }

    async fn create_annotation(server: &axum_test::TestServer, strategy_id: &str) -> String {
        let created = server
            .post("/api/annotations")
            .json(&json!({
                "strategy_id": strategy_id,
                "target_symbol": "7203",
                "target_kind": "observation",
                "timestamp": "2026-01-01T00:00:00Z",
                "text": "text",
            }))
            .await;
        created.assert_status(StatusCode::CREATED);
        created.json::<Value>()["id"]
            .as_str()
            .expect("id")
            .to_string()
    }

    #[sqlx::test(migrations = false)]
    async fn create_comment_with_anchor_text_computes_start_and_end_line(pool: PgPool) {
        let server = create_test_server(pool).await;
        let strategy_id = crate::testing::create_strategy(&server, "s").await;
        let note_id = create_note(
            &server,
            &strategy_id,
            indoc::indoc! {"
                line one
                line two
                line three"},
        )
        .await;

        let res = server
            .post("/api/comments")
            .json(&json!({
                "target_kind": "note",
                "target_id": note_id,
                "body": "fix this line",
                "anchor_text": "line two",
            }))
            .await;
        res.assert_status(StatusCode::CREATED);
        assert_eq!(
            normalize(res.json()),
            json!({
                "id": "<id>",
                "target_kind": "note",
                "target_id": "<target_id>",
                "parent_id": null,
                "body": "fix this line",
                "author_kind": "human",
                "author_label": "user",
                "resolved": false,
                "created_at": "<created_at>",
                "anchor_text": "line two",
                "start_line": 2,
                "end_line": 2,
                "drifted": false,
            }),
        );
    }

    #[sqlx::test(migrations = false)]
    async fn create_comment_with_missing_anchor_text_marks_drifted(pool: PgPool) {
        let server = create_test_server(pool).await;
        let strategy_id = crate::testing::create_strategy(&server, "s").await;
        let note_id = create_note(
            &server,
            &strategy_id,
            indoc::indoc! {"
                line one
                line two
                line three"},
        )
        .await;

        let res = server
            .post("/api/comments")
            .json(&json!({
                "target_kind": "note",
                "target_id": note_id,
                "body": "fix this line",
                "anchor_text": "line that no longer exists",
            }))
            .await;
        res.assert_status(StatusCode::CREATED);
        assert_eq!(
            normalize(res.json()),
            json!({
                "id": "<id>",
                "target_kind": "note",
                "target_id": "<target_id>",
                "parent_id": null,
                "body": "fix this line",
                "author_kind": "human",
                "author_label": "user",
                "resolved": false,
                "created_at": "<created_at>",
                "anchor_text": "line that no longer exists",
                "start_line": null,
                "end_line": null,
                "drifted": true,
            }),
        );
    }

    #[sqlx::test(migrations = false)]
    async fn create_comment_on_annotation_with_anchor_text_saves_text_without_line_numbers(
        pool: PgPool,
    ) {
        let server = create_test_server(pool).await;
        let strategy_id = crate::testing::create_strategy(&server, "s").await;
        let annotation_id = create_annotation(&server, &strategy_id).await;

        let res = server
            .post("/api/comments")
            .json(&json!({
                "target_kind": "annotation",
                "target_id": annotation_id,
                "body": "fix this",
                "anchor_text": "some selected text",
            }))
            .await;
        res.assert_status(StatusCode::CREATED);
        assert_eq!(
            normalize(res.json()),
            json!({
                "id": "<id>",
                "target_kind": "annotation",
                "target_id": "<target_id>",
                "parent_id": null,
                "body": "fix this",
                "author_kind": "human",
                "author_label": "user",
                "resolved": false,
                "created_at": "<created_at>",
                "anchor_text": "some selected text",
                "start_line": null,
                "end_line": null,
                "drifted": false,
            }),
        );
    }
}
