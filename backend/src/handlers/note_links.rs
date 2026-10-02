use axum::Json;
use axum::extract::State;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::AppState;
use crate::error::{AppError, ErrorResponse};
use crate::extractors::{JsonPath, JsonQuery};

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
    let links = state
        .note_read_use_cases
        .list_note_links(note_id, params.version_id, None)
        .await
        .map_err(crate::handlers::notes::map_note_read_error)?;
    Ok(Json(NoteLinksResponse {
        outgoing: links.outgoing.into_iter().map(Into::into).collect(),
        incoming: links.incoming.into_iter().map(Into::into).collect(),
    }))
}

impl From<core_application::note::NoteLinkView> for NoteLinkItem {
    fn from(link: core_application::note::NoteLinkView) -> Self {
        Self {
            note_id: link.note_id,
            version_id: link.version_id,
            version_no: link.version_no,
            title: link.title,
        }
    }
}
