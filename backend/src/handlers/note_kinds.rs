//! ノート種別 CRUD の REST handler。

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use core_application::change_history::Actor;
use core_application::note_kind::{CreateNoteKindCommand, UpdateNoteKindCommand};
use serde::Deserialize;
use utoipa::ToSchema;

use crate::AppState;
use crate::error::{AppError, ErrorResponse};
use crate::extractors::{JsonBody, JsonPath};
use crate::models::NoteKindResponse;

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateNoteKindRequest {
    pub key: String,
    pub display_name: String,
    pub requires_approval: bool,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub sort_order: Option<i32>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct UpdateNoteKindRequest {
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub requires_approval: Option<bool>,
    #[serde(
        default,
        deserialize_with = "crate::serde_helpers::deserialize_nullable_option"
    )]
    pub description: Option<Option<String>>,
    #[serde(default)]
    pub sort_order: Option<i32>,
}

/// ノート種別一覧
#[utoipa::path(
    get,
    path = "/api/note-kinds",
    tag = "note_kinds",
    responses(
        (status = 200, body = Vec<NoteKindResponse>),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn list_note_kinds(
    State(state): State<AppState>,
) -> Result<Json<Vec<NoteKindResponse>>, AppError> {
    Ok(Json(
        state
            .use_cases
            .note_kinds
            .list()
            .await?
            .into_iter()
            .map(Into::into)
            .collect(),
    ))
}

/// ノート種別を作成
#[utoipa::path(
    post,
    path = "/api/note-kinds",
    tag = "note_kinds",
    request_body = CreateNoteKindRequest,
    responses(
        (status = 201, body = NoteKindResponse),
        (status = 400, body = ErrorResponse),
        (status = 409, description = "key が既存と衝突", body = ErrorResponse),
        (status = 415, description = "Content-Type ヘッダが application/json ではない", body = ErrorResponse),
        (status = 422, description = "リクエストボディのパースに失敗", body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn create_note_kind(
    State(state): State<AppState>,
    JsonBody(payload): JsonBody<CreateNoteKindRequest>,
) -> Result<(StatusCode, Json<NoteKindResponse>), AppError> {
    let created = state
        .use_cases
        .note_kinds
        .create(
            Actor::Human,
            CreateNoteKindCommand {
                key: payload.key,
                display_name: payload.display_name,
                requires_approval: payload.requires_approval,
                description: payload.description,
                sort_order: payload.sort_order,
            },
        )
        .await?;
    Ok((StatusCode::CREATED, Json(created.into())))
}

/// ノート種別を部分更新する (key は変更不可)
#[utoipa::path(
    patch,
    path = "/api/note-kinds/{key}",
    tag = "note_kinds",
    params(("key" = String, Path, description = "ノート種別 key")),
    request_body = UpdateNoteKindRequest,
    responses(
        (status = 200, body = NoteKindResponse),
        (status = 400, body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 415, description = "Content-Type ヘッダが application/json ではない", body = ErrorResponse),
        (status = 422, description = "リクエストボディのパースに失敗", body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn update_note_kind(
    State(state): State<AppState>,
    JsonPath(key): JsonPath<String>,
    JsonBody(payload): JsonBody<UpdateNoteKindRequest>,
) -> Result<Json<NoteKindResponse>, AppError> {
    let updated = state
        .use_cases
        .note_kinds
        .update(
            Actor::Human,
            &key,
            UpdateNoteKindCommand {
                display_name: payload.display_name,
                requires_approval: payload.requires_approval,
                description: payload.description,
                sort_order: payload.sort_order,
            },
        )
        .await?;
    Ok(Json(updated.into()))
}

/// ノートが使用中の種別は削除しない
#[utoipa::path(
    delete,
    path = "/api/note-kinds/{key}",
    tag = "note_kinds",
    params(("key" = String, Path, description = "ノート種別 key")),
    responses(
        (status = 204),
        (status = 404, body = ErrorResponse),
        (status = 409, description = "既存のノートがこの種別を使用中", body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn delete_note_kind(
    State(state): State<AppState>,
    JsonPath(key): JsonPath<String>,
) -> Result<StatusCode, AppError> {
    state
        .use_cases
        .note_kinds
        .delete(Actor::Human, &key)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;
    use sea_orm::ActiveModelTrait;
    use sea_orm::ActiveValue::{Set, Unchanged};
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
    use serde_json::{Value, json};
    use uuid::Uuid;

    use gateway_postgres::entities::{change_history, note, note_kind, note_version};

    use crate::testing::{
        create_test_server, insert_test_note_with_execution_id, insert_test_strategy,
    };

    #[backend_test_macros::database_test]
    async fn disabling_note_kind_approval_approves_pending_versions_with_history(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let server = create_test_server(db.clone()).await;
        let kind_key = "sample-kind";
        note_kind::ActiveModel {
            key: Set(kind_key.to_string()),
            display_name: Set("Sample Kind".to_string()),
            requires_approval: Set(true),
            description: Set(None),
            sort_order: Set(0),
        }
        .insert(&db)
        .await
        .expect("insert note kind");
        let strategy_id = insert_test_strategy(&db, "Sample strategy").await;
        let note_id = insert_test_note_with_execution_id(
            &db,
            strategy_id,
            "Sample title",
            "Sample body",
            "sample-execution-id",
        )
        .await;
        note::ActiveModel {
            id: Unchanged(note_id),
            kind: Set(Some(kind_key.to_string())),
            ..Default::default()
        }
        .update(&db)
        .await
        .expect("set note kind");

        let response = server
            .patch(&format!("/api/note-kinds/{kind_key}"))
            .json(&json!({ "requires_approval": false }))
            .await;
        response.assert_status(StatusCode::OK);
        let response_body: Value = response.json();

        let version = note_version::Entity::find()
            .filter(note_version::Column::NoteId.eq(note_id))
            .one(&db)
            .await
            .expect("find note version")
            .expect("note version exists");
        let note_kind_history_id = Uuid::new_v5(
            &Uuid::NAMESPACE_OID,
            format!("note_kind:{kind_key}").as_bytes(),
        );
        let history = change_history::Entity::find()
            .filter(change_history::Column::TargetId.is_in([note_id, note_kind_history_id]))
            .all(&db)
            .await
            .expect("find change history")
            .into_iter()
            .filter(|record| record.op == "status_change" || record.op == "update")
            .map(|record| {
                let diff: Value = record.diff_json;
                let diff = if record.target_kind == "note" {
                    json!({
                        "from": diff["from"],
                        "to": diff["to"],
                        "version_id": "version-id",
                        "previous_current_version_id": "version-id",
                        "label": diff["label"],
                    })
                } else {
                    diff
                };
                (
                    record.target_kind,
                    record.actor_kind,
                    record.actor_label,
                    record.op,
                    diff,
                    record.summary,
                )
            })
            .collect::<Vec<_>>();
        let mut history = history;
        history.sort_by_key(|record| record.0.clone());

        assert_eq!(
            (response_body, (version.status, version.is_current), history,),
            (
                json!({
                    "key": kind_key,
                    "display_name": "Sample Kind",
                    "requires_approval": false,
                    "description": null,
                    "sort_order": 0,
                }),
                ("approved".to_string(), true),
                vec![
                    (
                        "note".to_string(),
                        "human".to_string(),
                        "user".to_string(),
                        "status_change".to_string(),
                        json!({
                            "from": "unread",
                            "to": "approved",
                            "version_id": "version-id",
                            "previous_current_version_id": "version-id",
                            "label": null,
                        }),
                        None,
                    ),
                    (
                        "note_kind".to_string(),
                        "human".to_string(),
                        "user".to_string(),
                        "update".to_string(),
                        json!({
                            "requires_approval": { "from": true, "to": false },
                        }),
                        Some(format!("updated note kind {kind_key}")),
                    ),
                ],
            ),
        );
    }
}
