use serde::Serialize;
use utoipa::ToSchema;

use super::{NoteResponse, NoteVersionResponse};

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct NoteOverviewResponse {
    pub representative: NoteResponse,
    pub current: Option<NoteVersionResponse>,
    pub latest_unread: Option<NoteVersionResponse>,
    pub unread_version_count: i64,
}

impl From<core_application::note::NoteOverview> for NoteOverviewResponse {
    fn from(overview: core_application::note::NoteOverview) -> Self {
        Self {
            representative: NoteResponse::from_snapshot(overview.representative),
            current: overview.current.map(Into::into),
            latest_unread: overview.latest_unread.map(Into::into),
            unread_version_count: overview.unread_version_count,
        }
    }
}
