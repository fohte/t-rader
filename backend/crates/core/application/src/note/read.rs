use std::collections::HashMap;

use thiserror::Error;
use uuid::Uuid;

use crate::markdown_lint::{MarkdownLintService, MarkdownLintServiceError, NOTE_DISPLAY};
use core_domain::markdown_lint::{FixError, apply_safe_edits};

use super::query::{NoteReadQueryError, SharedNoteReadQuery};
use super::types::{
    Note, NoteDisplaySnapshot, NoteLink, NoteLinks, NoteListPage, NoteListQuery, NoteOverview,
    NoteOverviewQuery, NoteSnapshot, NoteVersion, PendingNoteVersion,
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

#[derive(Debug, Error)]
enum NoteDisplayBodyError {
    #[error(transparent)]
    Inspect(#[from] MarkdownLintServiceError),
    #[error(transparent)]
    ApplyEdits(#[from] FixError),
}

#[derive(Clone)]
pub struct NoteReadUseCases {
    query: SharedNoteReadQuery,
    markdown_lint: MarkdownLintService,
}

impl NoteReadUseCases {
    #[cfg(test)]
    pub fn new(query: SharedNoteReadQuery) -> Self {
        Self {
            query,
            markdown_lint: MarkdownLintService::default(),
        }
    }

    pub fn with_markdown_lint(
        query: SharedNoteReadQuery,
        markdown_lint: MarkdownLintService,
    ) -> Self {
        Self {
            query,
            markdown_lint,
        }
    }

    pub async fn list_notes(
        &self,
        query: NoteListQuery,
    ) -> Result<NoteListPage, NoteReadUseCaseError> {
        Ok(self.query.list_notes(query).await?)
    }

    pub async fn list_note_overviews(
        &self,
        query: NoteOverviewQuery,
    ) -> Result<Vec<NoteOverview>, NoteReadUseCaseError> {
        Ok(self.query.list_note_overviews(query).await?)
    }

    pub async fn list_notes_written_by_task(
        &self,
        task_id: Uuid,
    ) -> Result<Vec<NoteSnapshot>, NoteReadUseCaseError> {
        Ok(self.query.list_notes_written_by_task(task_id).await?)
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

    pub async fn get_note_for_display(
        &self,
        note_id: Uuid,
        version_id: Option<Uuid>,
        use_latest_if_no_current: bool,
    ) -> Result<NoteDisplaySnapshot, NoteReadUseCaseError> {
        let snapshot = self
            .get_note(note_id, version_id, use_latest_if_no_current)
            .await?;
        let source = snapshot.version.body_md.clone();
        let display_body_md = match self.correct_display_body(&source).await {
            Ok(display_body_md) => display_body_md,
            Err(error) => {
                tracing::error!(
                    note_id = %note_id,
                    version_id = %snapshot.version.id,
                    error = %error,
                    "ノート表示本文の補正に失敗したため原文を返します"
                );
                source.clone()
            }
        };

        Ok(NoteDisplaySnapshot {
            snapshot,
            display_body_md,
        })
    }

    async fn correct_display_body(&self, source: &str) -> Result<String, NoteDisplayBodyError> {
        let report = self.markdown_lint.inspect(source, NOTE_DISPLAY).await?;
        let edits = report
            .diagnostics
            .into_iter()
            .filter_map(|diagnostic| diagnostic.fix)
            .collect::<Vec<_>>();
        Ok(apply_safe_edits(source, &edits)?)
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
    ) -> Result<Vec<PendingNoteVersion>, NoteReadUseCaseError> {
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

    pub async fn note_ids_for_versions(
        &self,
        version_ids: &[Uuid],
    ) -> Result<HashMap<Uuid, Uuid>, NoteReadUseCaseError> {
        Ok(self.query.find_note_ids_for_versions(version_ids).await?)
    }

    pub(crate) async fn require_note(&self, note_id: Uuid) -> Result<Note, NoteReadUseCaseError> {
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
    use rstest::rstest;

    use super::*;
    use crate::markdown_lint::MarkdownLintService;
    use crate::note::FakeNoteReadQuery;
    use crate::refs::{RefUseCases, SharedRefRepository, test_support::FakeRefRepository};
    use crate::unit_of_work::{UnitOfWork, UnitOfWorkError, UnitOfWorkTransaction};

    const NOTE_ID: Uuid = Uuid::from_u128(1);
    const VERSION_ID: Uuid = Uuid::from_u128(2);

    fn build_use_cases() -> (NoteReadUseCases, Arc<FakeNoteReadQuery>) {
        let query = Arc::new(FakeNoteReadQuery::new(NOTE_ID, VERSION_ID));
        (NoteReadUseCases::new(query.clone()), query)
    }

    struct ReadTestUnitOfWork;

    #[async_trait]
    impl UnitOfWork for ReadTestUnitOfWork {
        async fn begin(&self) -> Result<UnitOfWorkTransaction, UnitOfWorkError> {
            Ok(UnitOfWorkTransaction::new(()))
        }

        async fn commit(&self, _: UnitOfWorkTransaction) -> Result<(), UnitOfWorkError> {
            Ok(())
        }
    }

    fn lint_service(names: &[(&str, &str)], fail_stock_lookup: bool) -> MarkdownLintService {
        let repository = Arc::new(FakeRefRepository::new(
            names
                .iter()
                .map(|(id, name)| ((*id).into(), (*name).into()))
                .collect(),
            fail_stock_lookup,
        ));
        let shared_repository: SharedRefRepository = repository;
        let refs = RefUseCases::new(Arc::new(ReadTestUnitOfWork), shared_repository);
        MarkdownLintService::new(refs)
    }

    fn display_snapshot(query: &FakeNoteReadQuery, display_body_md: &str) -> NoteDisplaySnapshot {
        NoteDisplaySnapshot {
            snapshot: NoteSnapshot {
                note: query.note(),
                version: query.version(),
                created_by_kind: "human".into(),
            },
            display_body_md: display_body_md.into(),
        }
    }

    #[tokio::test]
    async fn get_note_returns_a_note_without_strategy_ownership() {
        let query = Arc::new(FakeNoteReadQuery::new(NOTE_ID, VERSION_ID));
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
    async fn get_note_returns_latest_version_when_current_is_missing_and_fallback_is_enabled() {
        let query = Arc::new(FakeNoteReadQuery::new_unreviewed(NOTE_ID, VERSION_ID));
        let use_cases = NoteReadUseCases::new(query.clone());

        assert_eq!(
            use_cases
                .get_note(NOTE_ID, None, true)
                .await
                .expect("the latest version is used when no current version exists"),
            NoteSnapshot {
                note: query.note(),
                version: query.version(),
                created_by_kind: "human".into(),
            },
        );
    }

    #[tokio::test]
    async fn get_note_for_display_fixes_only_resolved_duplicate_names_without_changing_raw_version()
    {
        let source = indoc::indoc! {"
            [[stock:DEMO-A]] 架空工業: 一つ目
            [[stock:DEMO-B]] 架空商事: 二つ目
            続く行
        "};
        let display_source = indoc::indoc! {"
            [[stock:DEMO-A]]: 一つ目
            [[stock:DEMO-B]]: 二つ目
            続く行
        "};
        let query = Arc::new(FakeNoteReadQuery::with_body(NOTE_ID, VERSION_ID, source));
        let use_cases = NoteReadUseCases::with_markdown_lint(
            query.clone(),
            lint_service(&[("DEMO-A", "架空工業"), ("DEMO-B", "架空商事")], false),
        );

        let first = use_cases
            .get_note_for_display(NOTE_ID, Some(VERSION_ID), false)
            .await
            .expect("historical version is readable");
        let second = use_cases
            .get_note_for_display(NOTE_ID, Some(VERSION_ID), false)
            .await
            .expect("repeated read succeeds");

        let expected = display_snapshot(&query, display_source);
        assert_eq!((first, second), (expected.clone(), expected));
    }

    #[rstest]
    #[case::unresolved_name(false)]
    #[case::resolver_failure(true)]
    #[tokio::test]
    async fn get_note_for_display_returns_raw_body_when_a_name_cannot_be_resolved(
        #[case] fail_stock_lookup: bool,
    ) {
        let source = "[[stock:DEMO-UNKNOWN]] 架空銘柄";
        let query = Arc::new(FakeNoteReadQuery::with_body(NOTE_ID, VERSION_ID, source));
        let use_cases = NoteReadUseCases::with_markdown_lint(
            query.clone(),
            lint_service(&[], fail_stock_lookup),
        );

        let result = use_cases
            .get_note_for_display(NOTE_ID, Some(VERSION_ID), false)
            .await
            .expect("display reads keep working when no display name is available");

        assert_eq!(result, display_snapshot(&query, source));
    }

    #[tokio::test]
    async fn list_notes_preserves_the_query() {
        let (use_cases, query) = build_use_cases();
        let requested_query = NoteListQuery::default();

        use_cases
            .list_notes(requested_query.clone())
            .await
            .expect("listing succeeds");

        assert_eq!(query.listed_queries().await, vec![requested_query],);
    }

    #[tokio::test]
    async fn list_notes_preserves_tag_filter() {
        let (use_cases, query) = build_use_cases();
        let requested_query = NoteListQuery {
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
