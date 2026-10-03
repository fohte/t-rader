use thiserror::Error;
use uuid::Uuid;

use crate::strategy_scope::StrategyScope;

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
        _scope: Option<StrategyScope>,
        query: NoteListQuery,
    ) -> Result<NoteListPage, NoteReadUseCaseError> {
        Ok(self.query.list_notes(query).await?)
    }

    pub async fn get_note(
        &self,
        note_id: Uuid,
        version_id: Option<Uuid>,
        use_latest_if_no_current: bool,
        _scope: Option<StrategyScope>,
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
        _scope: Option<StrategyScope>,
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

    pub async fn ensure_note_version_exists(
        &self,
        version_id: Uuid,
    ) -> Result<Note, NoteReadUseCaseError> {
        let note = self
            .query
            .find_note_for_version(version_id)
            .await?
            .ok_or(NoteReadUseCaseError::NoteVersionNotFound)?;
        Ok(note)
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

    use async_trait::async_trait;
    use chrono::{DateTime, FixedOffset, Utc};
    use rstest::rstest;
    use serde_json::json;
    use tokio::sync::Mutex;

    use super::*;
    use crate::note::query::{NoteReadQuery, NoteReadQueryError};
    use crate::note::types::NoteVersion;

    const NOTE_ID: Uuid = Uuid::from_u128(1);
    const VERSION_ID: Uuid = Uuid::from_u128(2);

    struct FakeNoteReadQuery {
        note: Note,
        listed_queries: Mutex<Vec<NoteListQuery>>,
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
            Ok((version_id == VERSION_ID).then(|| self.note.clone()))
        }

        async fn find_note_version(
            &self,
            note_id: Uuid,
            version_id: Option<Uuid>,
            _use_latest_if_no_current: bool,
        ) -> Result<Option<NoteVersion>, NoteReadQueryError> {
            Ok((note_id == self.note.id && version_id.is_none()).then(version))
        }

        async fn find_initial_created_by_kind(
            &self,
            note_id: Uuid,
        ) -> Result<Option<String>, NoteReadQueryError> {
            Ok((note_id == self.note.id).then(|| "human".into()))
        }

        async fn list_note_versions(
            &self,
            note_id: Uuid,
        ) -> Result<Vec<NoteVersion>, NoteReadQueryError> {
            Ok((note_id == self.note.id)
                .then(version)
                .into_iter()
                .collect())
        }

        async fn find_note_version_by_number(
            &self,
            note_id: Uuid,
            version_no: i32,
        ) -> Result<Option<NoteVersion>, NoteReadQueryError> {
            Ok((note_id == self.note.id && version_no == 1).then(version))
        }

        async fn list_pending_note_versions(&self) -> Result<Vec<NoteVersion>, NoteReadQueryError> {
            Ok(Vec::new())
        }

        async fn list_notes(
            &self,
            query: NoteListQuery,
        ) -> Result<NoteListPage, NoteReadQueryError> {
            self.listed_queries.lock().await.push(query);
            Ok(NoteListPage {
                notes: Vec::new(),
                cursor: None,
                has_more: false,
            })
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

    fn note() -> Note {
        Note {
            id: NOTE_ID,
            strategy_id: None,
            kind: None,
            trigger: None,
            trigger_label: None,
            created_at: timestamp(),
            updated_at: timestamp(),
            execution_id: None,
        }
    }

    fn version() -> NoteVersion {
        NoteVersion {
            id: VERSION_ID,
            note_id: NOTE_ID,
            version_no: 1,
            title: "Example note".into(),
            body_md: "body".into(),
            frontmatter_json: json!({}),
            graphs_json: json!([]),
            status: "approved".into(),
            is_current: true,
            change_reason: None,
            created_by_kind: "human".into(),
            execution_id: None,
            created_at: timestamp(),
            reviewed_at: None,
        }
    }

    fn timestamp() -> DateTime<FixedOffset> {
        DateTime::<Utc>::UNIX_EPOCH.fixed_offset()
    }

    fn build_use_cases() -> (NoteReadUseCases, Arc<FakeNoteReadQuery>) {
        let query = Arc::new(FakeNoteReadQuery {
            note: note(),
            listed_queries: Mutex::new(Vec::new()),
        });
        (NoteReadUseCases::new(query.clone()), query)
    }

    #[rstest]
    #[case::unassigned(None)]
    #[case::another_strategy(Some(Uuid::from_u128(4)))]
    #[tokio::test]
    async fn get_note_accepts_notes_with_any_strategy_owner(#[case] strategy_id: Option<Uuid>) {
        let mut target_note = note();
        target_note.strategy_id = strategy_id;
        let query = Arc::new(FakeNoteReadQuery {
            note: target_note.clone(),
            listed_queries: Mutex::new(Vec::new()),
        });
        let use_cases = NoteReadUseCases::new(query);

        let result = use_cases
            .get_note(NOTE_ID, None, false, Some(Uuid::from_u128(3).into()))
            .await
            .expect("a note is readable from a different strategy scope");

        assert_eq!(
            result,
            NoteSnapshot {
                note: target_note,
                version: version(),
                created_by_kind: "human".into(),
            },
        );
    }

    #[tokio::test]
    async fn list_notes_preserves_the_query_strategy_filter() {
        let (use_cases, query) = build_use_cases();
        let requested_query = NoteListQuery {
            strategy_id: Some(Uuid::from_u128(4)),
            ..NoteListQuery::default()
        };

        use_cases
            .list_notes(Some(Uuid::from_u128(3).into()), requested_query.clone())
            .await
            .expect("listing succeeds");

        assert_eq!(
            query.listed_queries.lock().await.clone(),
            vec![requested_query],
        );
    }
}
