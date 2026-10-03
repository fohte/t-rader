use thiserror::Error;
use uuid::Uuid;

use super::query::{NoteReadQueryError, SharedNoteReadQuery};
use super::types::{
    Note, NoteLink, NoteLinks, NoteListPage, NoteListQuery, NoteSnapshot, NoteVersion,
};

#[derive(Debug, Error)]
pub enum NoteReadUseCaseError {
    #[error("{0}")]
    NotFound(String),
    #[error("version_id {version_id} does not belong to note {note_id}")]
    VersionDoesNotBelong { note_id: Uuid, version_id: Uuid },
    #[error("note {0} has no version")]
    NoVersion(Uuid),
    #[error("initial version for note {0} not found")]
    InitialVersionNotFound(Uuid),
    #[error("note version {note_id}/{version_no} not found")]
    VersionNumberNotFound { note_id: Uuid, version_no: i32 },
    #[error("note version not found")]
    NoteVersionNotFound,
    #[error(transparent)]
    Query(#[from] NoteReadQueryError),
}

#[derive(Clone)]
pub struct NoteReadUseCases {
    query: SharedNoteReadQuery,
}

impl NoteReadUseCases {
    pub fn new(query: SharedNoteReadQuery) -> Self {
        Self { query }
    }

    pub async fn list_notes(
        &self,
        query: NoteListQuery,
    ) -> Result<NoteListPage, NoteReadUseCaseError> {
        Ok(self.query.list_notes(query).await?)
    }

    pub async fn get_note(
        &self,
        note_id: Uuid,
        version_id: Option<Uuid>,
        use_latest_if_no_current: bool,
    ) -> Result<NoteSnapshot, NoteReadUseCaseError> {
        let note = self.require_note(note_id).await?;
        let version = self
            .query
            .find_note_version(note_id, version_id, use_latest_if_no_current)
            .await?
            .ok_or_else(|| match version_id {
                Some(version_id) => NoteReadUseCaseError::VersionDoesNotBelong {
                    note_id,
                    version_id,
                },
                None if use_latest_if_no_current => NoteReadUseCaseError::NoVersion(note_id),
                None => NoteReadUseCaseError::NotFound(format!(
                    "current version for note {note_id} not found"
                )),
            })?;
        let created_by_kind = self
            .query
            .find_initial_created_by_kind(note_id)
            .await?
            .ok_or(NoteReadUseCaseError::InitialVersionNotFound(note_id))?;
        Ok(NoteSnapshot {
            note,
            version,
            created_by_kind,
        })
    }

    pub async fn get_note_strategy_id(
        &self,
        note_id: Uuid,
    ) -> Result<Option<Uuid>, NoteReadUseCaseError> {
        Ok(self.require_note(note_id).await?.strategy_id)
    }

    pub async fn list_note_versions(
        &self,
        note_id: Uuid,
    ) -> Result<Vec<NoteVersion>, NoteReadUseCaseError> {
        self.require_note(note_id).await?;
        Ok(self.query.list_note_versions(note_id).await?)
    }

    pub async fn get_note_version(
        &self,
        note_id: Uuid,
        version_no: i32,
    ) -> Result<NoteVersion, NoteReadUseCaseError> {
        self.query
            .find_note_version_by_number(note_id, version_no)
            .await?
            .ok_or(NoteReadUseCaseError::VersionNumberNotFound {
                note_id,
                version_no,
            })
    }

    pub async fn list_pending_note_versions(
        &self,
    ) -> Result<Vec<NoteVersion>, NoteReadUseCaseError> {
        Ok(self.query.list_pending_note_versions().await?)
    }

    pub async fn find_links_from_version(
        &self,
        version_id: Uuid,
    ) -> Result<Vec<NoteLink>, NoteReadUseCaseError> {
        Ok(self.query.find_links_from_version(version_id).await?)
    }

    pub async fn list_note_links(
        &self,
        note_id: Uuid,
        version_id: Option<Uuid>,
    ) -> Result<NoteLinks, NoteReadUseCaseError> {
        self.require_note(note_id).await?;
        let source_version = self
            .query
            .find_note_version(note_id, version_id, false)
            .await?
            .ok_or_else(|| match version_id {
                Some(version_id) => NoteReadUseCaseError::VersionDoesNotBelong {
                    note_id,
                    version_id,
                },
                None => NoteReadUseCaseError::NotFound(format!(
                    "current version for note {note_id} not found"
                )),
            })?;
        Ok(self
            .query
            .list_note_links(note_id, source_version.id)
            .await?)
    }

    pub async fn note_version_exists(
        &self,
        version_id: Uuid,
    ) -> Result<bool, NoteReadUseCaseError> {
        Ok(self
            .query
            .find_note_for_version(version_id)
            .await?
            .is_some())
    }

    async fn require_note(&self, note_id: Uuid) -> Result<Note, NoteReadUseCaseError> {
        self.query
            .find_note(note_id)
            .await?
            .ok_or_else(|| NoteReadUseCaseError::NotFound(format!("note {note_id} not found")))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use rstest::rstest;

    use super::*;
    use crate::note::FakeNoteReadQuery;

    const NOTE_ID: Uuid = Uuid::from_u128(1);
    const VERSION_ID: Uuid = Uuid::from_u128(2);

    fn build_use_cases() -> (NoteReadUseCases, Arc<FakeNoteReadQuery>) {
        let query = Arc::new(FakeNoteReadQuery::new(NOTE_ID, None, VERSION_ID));
        (NoteReadUseCases::new(query.clone()), query)
    }

    #[rstest]
    #[case::unassigned(None)]
    #[case::another_strategy(Some(Uuid::from_u128(4)))]
    #[tokio::test]
    async fn get_note_accepts_notes_with_any_strategy_owner(#[case] strategy_id: Option<Uuid>) {
        let query = Arc::new(FakeNoteReadQuery::new(NOTE_ID, strategy_id, VERSION_ID));
        let use_cases = NoteReadUseCases::new(query.clone());
        let target_note = query.note();

        let result = use_cases
            .get_note(NOTE_ID, None, false)
            .await
            .expect("a note is readable from a different strategy scope");

        assert_eq!(
            result,
            NoteSnapshot {
                note: target_note,
                version: query.version(),
                created_by_kind: "human".into(),
            },
        );
    }

    #[tokio::test]
    async fn list_notes_preserves_strategy_and_tag_filters() {
        let (use_cases, query) = build_use_cases();
        let requested_query = NoteListQuery {
            strategy_id: Some(Uuid::from_u128(4)),
            tag: Some("demo-focus".into()),
            ..NoteListQuery::default()
        };

        use_cases
            .list_notes(requested_query.clone())
            .await
            .expect("listing succeeds");

        assert_eq!(query.listed_queries().await, vec![requested_query],);
    }
}
