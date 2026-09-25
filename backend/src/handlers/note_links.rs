use std::collections::HashMap;

use axum::Json;
use axum::extract::State;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::AppState;
use crate::entities::{note, note_version};
use crate::error::{AppError, ErrorResponse};
use crate::extractors::{JsonPath, JsonQuery};
use crate::services::note_links::{find_current_links_to_note, find_links_from_version};
use crate::services::note_versions::{find_current_versions, find_version_of_note};

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct GetNoteLinksQuery {
    /// 省略時はノートの現行バージョンから出るリンクを返す。
    pub version_id: Option<Uuid>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct NoteLinkItem {
    /// 出リンクでは参照先、被リンクでは参照元のノート ID。
    pub note_id: Uuid,
    /// 出リンクでは固定先のバージョン ID、被リンクでは現行の参照元バージョン ID。
    /// `null` は参照先の現行バージョンへの追従を表す。
    pub version_id: Option<Uuid>,
    pub version_no: Option<i32>,
    pub title: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct NoteLinksResponse {
    pub outgoing: Vec<NoteLinkItem>,
    pub incoming: Vec<NoteLinkItem>,
}

/// ノートのバージョンから出るリンクと、現行バージョンからの被リンクを返す。
#[utoipa::path(
    get,
    path = "/api/notes/{id}/links",
    tag = "notes",
    params(
        ("id" = Uuid, Path, description = "ノート ID"),
        GetNoteLinksQuery,
    ),
    responses(
        (status = 200, body = NoteLinksResponse),
        (status = 400, body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn get_note_links(
    State(state): State<AppState>,
    JsonPath(note_id): JsonPath<Uuid>,
    JsonQuery(params): JsonQuery<GetNoteLinksQuery>,
) -> Result<Json<NoteLinksResponse>, AppError> {
    note::Entity::find_by_id(note_id)
        .one(&state.db)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("note {note_id} not found")))?;

    let source_version = find_version_of_note(&state.db, note_id, params.version_id)
        .await?
        .ok_or_else(|| match params.version_id {
            Some(version_id) => {
                AppError::NotFound(format!("version {version_id} for note {note_id} not found"))
            }
            None => AppError::NotFound(format!("current version for note {note_id} not found")),
        })?;

    let outgoing_links = find_links_from_version(&state.db, source_version.id).await?;
    let target_note_ids = outgoing_links
        .iter()
        .map(|link| link.to_note_id)
        .collect::<Vec<_>>();
    let current_targets = find_current_versions(&state.db, &target_note_ids).await?;
    let pinned_version_ids = outgoing_links
        .iter()
        .filter_map(|link| link.to_version_id)
        .collect::<Vec<_>>();
    let pinned_versions = if pinned_version_ids.is_empty() {
        Vec::new()
    } else {
        note_version::Entity::find()
            .filter(note_version::Column::Id.is_in(pinned_version_ids))
            .all(&state.db)
            .await?
    };
    let pinned_by_id: HashMap<Uuid, note_version::Model> = pinned_versions
        .into_iter()
        .map(|version| (version.id, version))
        .collect();
    let mut outgoing = outgoing_links
        .into_iter()
        .map(|link| {
            let resolved = match link.to_version_id {
                Some(version_id) => pinned_by_id.get(&version_id),
                None => current_targets.get(&link.to_note_id),
            };
            NoteLinkItem {
                note_id: link.to_note_id,
                version_id: link.to_version_id,
                version_no: resolved.map(|version| version.version_no),
                title: resolved.map(|version| version.title.clone()),
            }
        })
        .collect::<Vec<_>>();
    outgoing.sort_by_key(|link| link.note_id);

    let incoming_links = find_current_links_to_note(&state.db, note_id).await?;
    let source_version_ids = incoming_links
        .iter()
        .map(|link| link.from_version_id)
        .collect::<Vec<_>>();
    let source_versions = if source_version_ids.is_empty() {
        Vec::new()
    } else {
        note_version::Entity::find()
            .filter(note_version::Column::Id.is_in(source_version_ids))
            .filter(note_version::Column::IsCurrent.eq(true))
            .all(&state.db)
            .await?
    };
    let source_by_id: HashMap<Uuid, note_version::Model> = source_versions
        .into_iter()
        .map(|version| (version.id, version))
        .collect();
    let mut incoming = incoming_links
        .into_iter()
        .filter_map(|link| source_by_id.get(&link.from_version_id))
        .map(|version| NoteLinkItem {
            note_id: version.note_id,
            version_id: Some(version.id),
            version_no: Some(version.version_no),
            title: Some(version.title.clone()),
        })
        .collect::<Vec<_>>();
    incoming.sort_by_key(|link| link.note_id);

    Ok(Json(NoteLinksResponse { outgoing, incoming }))
}
