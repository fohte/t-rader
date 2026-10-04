use async_trait::async_trait;
use chrono::{DateTime, FixedOffset, Utc};
use serde_json::json;
use tokio::sync::Mutex;
use uuid::Uuid;

use super::query::{NoteReadQuery, NoteReadQueryError};
use super::types::{Note, NoteLink, NoteLinks, NoteListPage, NoteListQuery, NoteVersion};

pub struct FakeNoteReadQuery {
    note: Note,
    version: NoteVersion,
    listed_queries: Mutex<Vec<NoteListQuery>>,
}

impl FakeNoteReadQuery {
    pub fn new(note_id: Uuid, version_id: Uuid) -> Self {
        let timestamp = timestamp();
        Self {
            note: Note {
                id: note_id,
                kind: None,
                trigger: None,
                trigger_label: None,
                created_at: timestamp,
                updated_at: timestamp,
                execution_id: None,
            },
            version: NoteVersion {
                id: version_id,
                note_id,
                version_no: 1,
                title: "Example note".into(),
                body_md: "body".into(),
                frontmatter_json: json!({}),
                graphs_json: json!([]),
                resolved_price_references_json: json!({}),
                status: "approved".into(),
                is_current: true,
                change_reason: None,
                created_by_kind: "human".into(),
                execution_id: None,
                created_at: timestamp,
                reviewed_at: None,
            },
            listed_queries: Mutex::new(Vec::new()),
        }
    }

    pub fn note(&self) -> Note {
        self.note.clone()
    }

    pub fn version(&self) -> NoteVersion {
        self.version.clone()
    }

    pub async fn listed_queries(&self) -> Vec<NoteListQuery> {
        self.listed_queries.lock().await.clone()
    }
}

#[async_trait]
impl NoteReadQuery for FakeNoteReadQuery {
    async fn find_note(&self, note_id: Uuid) -> Result<Option<Note>, NoteReadQueryError> {
        Ok((self.note.id == note_id).then(|| self.note.clone()))
    }

    async fn find_note_for_version(
        &self,
        version_id: Uuid,
    ) -> Result<Option<Note>, NoteReadQueryError> {
        Ok((self.version.id == version_id).then(|| self.note.clone()))
    }

    async fn find_note_version(
        &self,
        note_id: Uuid,
        version_id: Option<Uuid>,
        _use_latest_if_no_current: bool,
    ) -> Result<Option<NoteVersion>, NoteReadQueryError> {
        Ok(
            (self.note.id == note_id && version_id.is_none_or(|id| id == self.version.id))
                .then(|| self.version.clone()),
        )
    }

    async fn find_initial_created_by_kind(
        &self,
        note_id: Uuid,
    ) -> Result<Option<String>, NoteReadQueryError> {
        Ok((self.note.id == note_id).then(|| "human".into()))
    }

    async fn list_note_versions(
        &self,
        note_id: Uuid,
    ) -> Result<Vec<NoteVersion>, NoteReadQueryError> {
        Ok((self.note.id == note_id)
            .then(|| self.version.clone())
            .into_iter()
            .collect())
    }

    async fn find_note_version_by_number(
        &self,
        note_id: Uuid,
        version_no: i32,
    ) -> Result<Option<NoteVersion>, NoteReadQueryError> {
        Ok(
            (self.note.id == note_id && self.version.version_no == version_no)
                .then(|| self.version.clone()),
        )
    }

    async fn list_pending_note_versions(&self) -> Result<Vec<NoteVersion>, NoteReadQueryError> {
        Ok(Vec::new())
    }

    async fn list_notes(&self, query: NoteListQuery) -> Result<NoteListPage, NoteReadQueryError> {
        self.listed_queries.lock().await.push(query);
        Ok(NoteListPage {
            notes: Vec::new(),
            cursor: None,
            has_more: false,
        })
    }

    async fn list_notes_written_by_task(
        &self,
        _task_id: Uuid,
    ) -> Result<Vec<super::types::NoteSnapshot>, NoteReadQueryError> {
        Ok(Vec::new())
    }

    async fn find_links_from_version(
        &self,
        _version_id: Uuid,
    ) -> Result<Vec<NoteLink>, NoteReadQueryError> {
        Ok(Vec::new())
    }

    async fn list_note_links(
        &self,
        _note_id: Uuid,
        _source_version_id: Uuid,
    ) -> Result<NoteLinks, NoteReadQueryError> {
        Ok(NoteLinks {
            outgoing: Vec::new(),
            incoming: Vec::new(),
        })
    }
}

fn timestamp() -> DateTime<FixedOffset> {
    DateTime::<Utc>::UNIX_EPOCH.fixed_offset()
}
