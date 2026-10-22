use axum::Json;
use axum::extract::State;
use serde::Deserialize;
use utoipa::IntoParams;

use crate::FrontendApiState;
use crate::error::AppError;
use crate::extractors::JsonQuery;
use crate::handlers::notes::map_note_read_error;
use crate::models::NoteOverviewResponse;
use core_application::note::{NoteOverviewQuery, NoteOverviewView};

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ListNoteOverviewsQuery {
    /// 表示対象。`all` は現行優先、`pending` は最新 unread のみを返す。
    #[param(example = "all")]
    pub view: Option<String>,
    /// ノート属性で絞り込む。
    pub kind: Option<String>,
    /// view が選んだ代表バージョンのタグで絞り込む。
    pub tag: Option<String>,
}

/// ノート一覧の代表・現行・最新 unread バージョン
#[utoipa::path(
    get,
    path = "/api/notes/overview",
    tag = "notes",
    params(ListNoteOverviewsQuery),
    responses(
        (status = 200, body = Vec<NoteOverviewResponse>),
        (status = 400, description = "リクエストパラメータが不正", body = crate::error::ErrorResponse),
        (status = 500, body = crate::error::ErrorResponse),
    )
)]
pub async fn list_note_overviews(
    State(state): State<FrontendApiState>,
    JsonQuery(params): JsonQuery<ListNoteOverviewsQuery>,
) -> Result<Json<Vec<NoteOverviewResponse>>, AppError> {
    let view = match params.view.as_deref().filter(|view| !view.is_empty()) {
        None | Some("all") => NoteOverviewView::All,
        Some("pending") => NoteOverviewView::Pending,
        Some(_) => return Err(AppError::Validation("view must be all or pending".into())),
    };
    let overviews = state
        .note_read_use_cases
        .list_note_overviews(NoteOverviewQuery {
            view,
            kind: params.kind.filter(|kind| !kind.is_empty()),
            tag: params.tag.filter(|tag| !tag.is_empty()),
        })
        .await
        .map_err(map_note_read_error)?;
    Ok(Json(overviews.into_iter().map(Into::into).collect()))
}
