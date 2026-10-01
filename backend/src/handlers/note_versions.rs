use axum::Json;
use axum::extract::State;
use core_application::strategy_task::TaskSource;
use uuid::Uuid;

use crate::AppState;
use crate::error::{AppError, ErrorResponse};
use crate::extractors::{JsonBody, JsonPath};
use crate::handlers::comments::map_comment_read_error;
use crate::handlers::strategies::map_submit_error;
use crate::models::{ChangeStatusRequest, NoteVersionResponse};
use core_application::note::INITIAL_NOTE_STATUS;

/// ノートの全バージョンを古い順に返す。
#[utoipa::path(
    get,
    path = "/api/notes/{id}/versions",
    tag = "notes",
    params(("id" = Uuid, Path, description = "ノート ID")),
    responses(
        (status = 200, body = Vec<NoteVersionResponse>),
        (status = 400, description = "リクエストパラメータが不正", body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn list_note_versions(
    State(state): State<AppState>,
    JsonPath(note_id): JsonPath<Uuid>,
) -> Result<Json<Vec<NoteVersionResponse>>, AppError> {
    let versions = state
        .use_cases
        .note_reads()
        .list_note_versions(note_id)
        .await
        .map_err(crate::handlers::notes::map_note_read_error)?;
    Ok(Json(versions.into_iter().map(Into::into).collect()))
}

/// ノートの指定バージョンを返す。
#[utoipa::path(
    get,
    path = "/api/notes/{id}/versions/{n}",
    tag = "notes",
    params(
        ("id" = Uuid, Path, description = "ノート ID"),
        ("n" = i32, Path, description = "バージョン番号"),
    ),
    responses(
        (status = 200, body = NoteVersionResponse),
        (status = 400, description = "リクエストパラメータが不正", body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn get_note_version(
    State(state): State<AppState>,
    JsonPath((note_id, version_no)): JsonPath<(Uuid, i32)>,
) -> Result<Json<NoteVersionResponse>, AppError> {
    Ok(Json(
        state
            .use_cases
            .note_reads()
            .get_note_version(note_id, version_no)
            .await
            .map_err(crate::handlers::notes::map_note_read_error)?
            .into(),
    ))
}

/// 承認待ちバージョンを作成日時順に返す。
#[utoipa::path(
    get,
    path = "/api/note-versions/pending",
    tag = "notes",
    responses(
        (status = 200, body = Vec<NoteVersionResponse>),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn list_pending_note_versions(
    State(state): State<AppState>,
) -> Result<Json<Vec<NoteVersionResponse>>, AppError> {
    let versions = state
        .use_cases
        .note_reads()
        .list_pending_note_versions()
        .await
        .map_err(crate::handlers::notes::map_note_read_error)?;
    Ok(Json(versions.into_iter().map(Into::into).collect()))
}

fn ensure_pending_version(version: &core_application::note::NoteVersion) -> Result<(), AppError> {
    if version.status != INITIAL_NOTE_STATUS {
        return Err(AppError::Conflict(format!(
            "note version {}/{} is not pending",
            version.note_id, version.version_no
        )));
    }
    Ok(())
}

/// 承認待ちバージョンを承認し、現行バージョンにする。
#[utoipa::path(
    post,
    path = "/api/notes/{id}/versions/{n}/approve",
    tag = "notes",
    params(
        ("id" = Uuid, Path, description = "ノート ID"),
        ("n" = i32, Path, description = "バージョン番号"),
    ),
    request_body = ChangeStatusRequest,
    responses(
        (status = 200, body = NoteVersionResponse),
        (status = 400, description = "リクエストパラメータが不正", body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 409, description = "バージョンが承認待ちではない", body = ErrorResponse),
        (status = 415, description = "Content-Type ヘッダが application/json ではない", body = ErrorResponse),
        (status = 422, description = "リクエストボディのパースに失敗", body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn approve_note_version(
    State(state): State<AppState>,
    JsonPath((note_id, version_no)): JsonPath<(Uuid, i32)>,
    JsonBody(payload): JsonBody<ChangeStatusRequest>,
) -> Result<Json<NoteVersionResponse>, AppError> {
    let updated = state
        .use_cases
        .notes()
        .approve_version(note_id, version_no, payload.label)
        .await
        .map_err(crate::handlers::notes::map_note_error)?;
    Ok(Json(updated.into()))
}

/// 承認待ちバージョンを却下する。
#[utoipa::path(
    post,
    path = "/api/notes/{id}/versions/{n}/reject",
    tag = "notes",
    params(
        ("id" = Uuid, Path, description = "ノート ID"),
        ("n" = i32, Path, description = "バージョン番号"),
    ),
    request_body = ChangeStatusRequest,
    responses(
        (status = 200, body = NoteVersionResponse),
        (status = 400, description = "リクエストパラメータが不正、または却下理由が必要", body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 409, description = "バージョンが承認待ちではない", body = ErrorResponse),
        (status = 415, description = "Content-Type ヘッダが application/json ではない", body = ErrorResponse),
        (status = 422, description = "リクエストボディのパースに失敗", body = ErrorResponse),
        (status = 500, body = ErrorResponse),
        (status = 503, description = "agent task client が未設定、または agent_config が見つからない", body = ErrorResponse),
    )
)]
pub async fn reject_note_version(
    State(state): State<AppState>,
    JsonPath((note_id, version_no)): JsonPath<(Uuid, i32)>,
    JsonBody(payload): JsonBody<ChangeStatusRequest>,
) -> Result<Json<NoteVersionResponse>, AppError> {
    let version = state
        .use_cases
        .note_reads()
        .get_note_version(note_id, version_no)
        .await
        .map_err(crate::handlers::notes::map_note_read_error)?;
    ensure_pending_version(&version)?;

    let line_comment_count = state
        .use_cases
        .comment_reads()
        .count_line_comments(version.id)
        .await
        .map_err(map_comment_read_error)?;
    let label = payload
        .label
        .as_deref()
        .map(str::trim)
        .filter(|label| !label.is_empty())
        .map(str::to_string);
    if line_comment_count == 0 && label.is_none() {
        return Err(AppError::Validation(
            "a rejection reason is required when no line comments are attached".into(),
        ));
    }

    let strategy_id = state
        .use_cases
        .note_reads()
        .get_note_strategy_id(note_id)
        .await
        .map_err(crate::handlers::notes::map_note_read_error)?;
    if let Some(strategy_id) = strategy_id {
        let reason = label
            .as_deref()
            .map(|label| format!("理由: {label}。"))
            .unwrap_or_default();
        let prompt = format!(
            "ノート「{}」(id: {}) の v{} (version_id: {}) がレビューで却下されました。{}付いているコメントを確認し、指摘を反映してください。",
            version.title, note_id, version_no, version.id, reason
        );
        state
            .use_cases
            .strategy_tasks()
            .submit_task(
                state.agent_task_client.as_ref(),
                strategy_id,
                &prompt,
                TaskSource::Review,
                None,
            )
            .await
            .map_err(map_submit_error)?;
    }

    let updated = state
        .use_cases
        .notes()
        .reject_version(note_id, version_no, label, line_comment_count > 0)
        .await
        .map_err(crate::handlers::notes::map_note_error)?;
    Ok(Json(updated.into()))
}

/// 承認済みの過去バージョンを現行にする。
#[utoipa::path(
    post,
    path = "/api/notes/{id}/versions/{n}/make-current",
    tag = "notes",
    params(
        ("id" = Uuid, Path, description = "ノート ID"),
        ("n" = i32, Path, description = "バージョン番号"),
    ),
    responses(
        (status = 200, body = NoteVersionResponse),
        (status = 400, description = "リクエストパラメータが不正", body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 409, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn make_note_version_current(
    State(state): State<AppState>,
    JsonPath((note_id, version_no)): JsonPath<(Uuid, i32)>,
) -> Result<Json<NoteVersionResponse>, AppError> {
    let updated = state
        .use_cases
        .notes()
        .make_version_current(note_id, version_no)
        .await
        .map_err(crate::handlers::notes::map_note_error)?;
    Ok(Json(updated.into()))
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;
    use serde_json::{Value, json};

    use crate::testing::create_test_server;

    fn normalize_note_version(mut value: Value) -> Value {
        for key in ["id", "note_id", "created_at", "reviewed_at"] {
            if let Some(value) = value.as_object_mut().and_then(|object| object.get_mut(key)) {
                *value = Value::String(format!("<{key}>"));
            }
        }
        value
    }

    async fn create_pending_note(
        server: &axum_test::TestServer,
        title: &str,
        body_md: &str,
    ) -> (String, String, i64) {
        let response = server
            .post("/api/notes")
            .json(&json!({
                "title": title,
                "body_md": body_md,
                "created_by_kind": "llm",
                "status": "unread",
            }))
            .await;
        response.assert_status(StatusCode::CREATED);
        let note = response.json::<Value>();
        (
            note["id"].as_str().expect("note id").to_string(),
            note["version_id"].as_str().expect("version id").to_string(),
            note["version_no"].as_i64().expect("version number"),
        )
    }

    #[backend_test_macros::database_test]
    async fn reject_note_version_counts_only_line_anchored_comments(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let server = create_test_server(db).await;
        let (plain_note_id, plain_version_id, plain_version_no) =
            create_pending_note(&server, "sample note", "sample body").await;
        let anchored_body = indoc::indoc! {"
            line one
            line two"};
        let (anchored_note_id, anchored_version_id, anchored_version_no) =
            create_pending_note(&server, "anchored sample note", anchored_body).await;
        server
            .post("/api/comments")
            .json(&json!({
                "target_kind": "note_version",
                "target_id": plain_version_id,
                "body": "sample comment",
            }))
            .await
            .assert_status(StatusCode::CREATED);
        server
            .post("/api/comments")
            .json(&json!({
                "target_kind": "note_version",
                "target_id": anchored_version_id,
                "body": "sample line comment",
                "anchor_text": "line two",
                "anchor_side": "new",
                "start_line": 2,
                "end_line": 2,
            }))
            .await
            .assert_status(StatusCode::CREATED);

        let plain_rejection = server
            .post(&format!(
                "/api/notes/{plain_note_id}/versions/{plain_version_no}/reject"
            ))
            .json(&json!({}))
            .await;
        let anchored_rejection = server
            .post(&format!(
                "/api/notes/{anchored_note_id}/versions/{anchored_version_no}/reject"
            ))
            .json(&json!({}))
            .await;
        assert_eq!(
            (
                plain_rejection.status_code(),
                plain_rejection.json::<Value>(),
                anchored_rejection.status_code(),
                normalize_note_version(anchored_rejection.json()),
            ),
            (
                StatusCode::BAD_REQUEST,
                json!({
                    "error": "a rejection reason is required when no line comments are attached"
                }),
                StatusCode::OK,
                json!({
                    "id": "<id>",
                    "note_id": "<note_id>",
                    "version_no": anchored_version_no,
                    "title": "anchored sample note",
                    "body_md": anchored_body,
                    "frontmatter_json": {},
                    "graphs_json": [],
                    "status": "rejected",
                    "is_current": true,
                    "change_reason": null,
                    "created_by_kind": "llm",
                    "execution_id": null,
                    "created_at": "<created_at>",
                    "reviewed_at": "<reviewed_at>",
                }),
            ),
        );
    }
}
