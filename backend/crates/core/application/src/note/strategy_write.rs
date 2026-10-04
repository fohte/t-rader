use chrono::Utc;
use serde_json::json;
use uuid::Uuid;

use crate::note::frontmatter_validation::ensure_frontmatter_tags_are_strings;
use crate::note::types::{
    NewNote, NoteSnapshot, NoteVersion, NoteVersionUpdate, NoteWriteCommand, NoteWriteResult,
};
use crate::note::use_cases::ensure_frontmatter_object;
use crate::note::version_write::AppendVersionCommand;
use crate::note::{INITIAL_NOTE_STATUS, NoteUseCaseError, NoteUseCases};
use crate::unit_of_work::UnitOfWorkTransaction;

const ALLOWED_STATUSES: [&str; 3] = ["approved", "unread", "rejected"];
const ALLOWED_CREATED_BY: [&str; 2] = ["human", "llm"];

impl NoteUseCases {
    pub async fn write(
        &self,
        command: NoteWriteCommand,
    ) -> Result<NoteWriteResult, NoteUseCaseError> {
        if let Some(frontmatter_json) = command.frontmatter_json.as_ref() {
            ensure_frontmatter_object(frontmatter_json)?;
            ensure_frontmatter_tags_are_strings(frontmatter_json)?;
        }
        if let Some(graphs_json) = command.graphs_json.as_ref() {
            let graphs: Vec<core_domain::note_graph::GraphDef> =
                serde_json::from_value(graphs_json.clone()).map_err(|error| {
                    NoteUseCaseError::Validation(format!("invalid graphs_json: {error}"))
                })?;
            core_domain::note_graph::validate_graphs(&graphs)
                .map_err(|error| NoteUseCaseError::Validation(error.to_string()))?;
        }

        if let Some(note_id) = command.note_id {
            let transaction = self.unit_of_work.begin().await?;
            let note = self.require_note(&transaction, note_id).await?;
            let result = self
                .update_strategy_note(&transaction, note, command)
                .await?;
            self.unit_of_work.commit(transaction).await?;
            return Ok(result);
        }

        let transaction = self.unit_of_work.begin().await?;
        if let Some(execution_id) = command.execution_id.as_deref()
            && let Some(note) = self
                .repository
                .find_note_by_execution_id(&transaction, execution_id)
                .await?
        {
            let result = self
                .update_strategy_note(&transaction, note, command)
                .await?;
            self.unit_of_work.commit(transaction).await?;
            return Ok(result);
        }

        self.validate_create_command(&command)?;
        let title = command
            .title
            .as_deref()
            .map(str::trim)
            .filter(|title| !title.is_empty())
            .ok_or_else(|| {
                NoteUseCaseError::Validation(if command.title.is_some() {
                    "title must not be empty".into()
                } else {
                    "title is required when creating a new note".into()
                })
            })?
            .to_string();
        let kind = command.kind.clone().flatten();
        if let Some(kind) = kind.as_deref() {
            self.ensure_note_kind_exists(&transaction, kind).await?;
        }

        let note_id = Uuid::new_v4();
        let inserted = self
            .repository
            .insert_note(
                &transaction,
                NewNote {
                    id: note_id,
                    kind,
                    trigger: command.trigger.clone(),
                    trigger_label: command.trigger_label.clone(),
                    execution_id: command.execution_id.clone(),
                },
            )
            .await?;
        let Some(note) = inserted else {
            let execution_id = command.execution_id.as_deref().ok_or_else(|| {
                NoteUseCaseError::Conflict(format!("note {note_id} was not inserted"))
            })?;
            let note = self
                .repository
                .find_note_by_execution_id(&transaction, execution_id)
                .await?
                .ok_or_else(|| {
                    NoteUseCaseError::Conflict(
                        "note disappeared after execution_id conflict".into(),
                    )
                })?;
            let result = self
                .update_strategy_note(&transaction, note, command)
                .await?;
            self.unit_of_work.commit(transaction).await?;
            return Ok(result);
        };

        let graphs_json = command.graphs_json.unwrap_or_else(|| json!([]));
        let frontmatter_json = command.frontmatter_json.unwrap_or_else(|| json!({}));
        let status = command.status.as_deref().unwrap_or_else(|| {
            if command.created_by_kind == "human" {
                "approved"
            } else {
                INITIAL_NOTE_STATUS
            }
        });
        let mut version = self
            .append_version(
                &transaction,
                &note,
                AppendVersionCommand {
                    title,
                    body_md: command.body_md.unwrap_or_default(),
                    frontmatter_json,
                    graphs_json,
                    created_by_kind: command.created_by_kind.clone(),
                    execution_id: command.execution_id,
                    change_reason: command.change_reason,
                    change_diff: command.change_diff,
                    actor: command.actor,
                },
            )
            .await?;
        if command.status.is_some()
            && command.created_by_kind != "human"
            && !version.is_current
            && status != INITIAL_NOTE_STATUS
        {
            return Err(NoteUseCaseError::Validation(
                "approval-required note versions must start as unread".into(),
            ));
        }
        if command.created_by_kind != "human" && version.is_current && status != INITIAL_NOTE_STATUS
        {
            version = self
                .repository
                .update_version(
                    &transaction,
                    NoteVersionUpdate {
                        id: version.id,
                        is_current: None,
                        status: Some(status.to_string()),
                        reviewed_at: Some(Utc::now().fixed_offset()),
                    },
                )
                .await?;
        }

        let result = self
            .write_result(&transaction, note_id, version, true)
            .await?;
        self.unit_of_work.commit(transaction).await?;
        Ok(result)
    }

    fn validate_create_command(&self, command: &NoteWriteCommand) -> Result<(), NoteUseCaseError> {
        if !ALLOWED_CREATED_BY.contains(&command.created_by_kind.as_str()) {
            return Err(NoteUseCaseError::Validation(format!(
                "invalid created_by_kind: {}",
                command.created_by_kind
            )));
        }
        let status = command.status.as_deref().unwrap_or_else(|| {
            if command.created_by_kind == "human" {
                "approved"
            } else {
                INITIAL_NOTE_STATUS
            }
        });
        if !ALLOWED_STATUSES.contains(&status) {
            return Err(NoteUseCaseError::Validation(format!(
                "invalid status: {status}"
            )));
        }
        if command.status.is_some() && command.created_by_kind == "human" && status != "approved" {
            return Err(NoteUseCaseError::Validation(
                "human-created notes must start as approved".into(),
            ));
        }
        Ok(())
    }

    async fn update_strategy_note(
        &self,
        transaction: &UnitOfWorkTransaction,
        note: crate::note::types::Note,
        command: NoteWriteCommand,
    ) -> Result<NoteWriteResult, NoteUseCaseError> {
        let current_version = self
            .repository
            .find_latest_version(transaction, note.id)
            .await?
            .ok_or_else(|| {
                NoteUseCaseError::Conflict(format!("note {} has no version", note.id))
            })?;
        let mut touched = false;
        let mut title = current_version.title.clone();
        let mut body_md = current_version.body_md.clone();
        let mut frontmatter_json = current_version.frontmatter_json.clone();
        let mut graphs_json = current_version.graphs_json.clone();
        if let Some(requested_title) = command.title {
            title = requested_title.trim().to_string();
            if title.is_empty() {
                return Err(NoteUseCaseError::Validation(
                    "title must not be empty".into(),
                ));
            }
            touched = true;
        }
        if let Some(body) = command.body_md {
            body_md = body;
            touched = true;
        }
        if let Some(kind) = command.kind
            && kind != note.kind
        {
            return Err(NoteUseCaseError::Validation(
                "kind can only be set when creating a new note".into(),
            ));
        }
        if let Some(frontmatter) = command.frontmatter_json {
            frontmatter_json = frontmatter;
            touched = true;
        }
        if let Some(graphs) = command.graphs_json {
            graphs_json = graphs;
            touched = true;
        }
        if !touched {
            return Err(NoteUseCaseError::Validation(
                "at least one of title / body_md / frontmatter_json / graphs must be provided"
                    .into(),
            ));
        }
        if title == current_version.title
            && body_md == current_version.body_md
            && frontmatter_json == current_version.frontmatter_json
            && graphs_json == current_version.graphs_json
        {
            return self
                .write_result(transaction, note.id, current_version, false)
                .await;
        }

        let version = self
            .append_version(
                transaction,
                &note,
                AppendVersionCommand {
                    title,
                    body_md,
                    frontmatter_json,
                    graphs_json,
                    created_by_kind: command.created_by_kind,
                    execution_id: command.execution_id,
                    change_reason: command.change_reason,
                    change_diff: None,
                    actor: command.actor,
                },
            )
            .await?;
        self.write_result(transaction, note.id, version, false)
            .await
    }

    async fn write_result(
        &self,
        transaction: &UnitOfWorkTransaction,
        note_id: Uuid,
        version: NoteVersion,
        created: bool,
    ) -> Result<NoteWriteResult, NoteUseCaseError> {
        let note = self.require_note(transaction, note_id).await?;
        let created_by_kind = self
            .repository
            .find_initial_created_by_kind(transaction, note_id)
            .await?
            .ok_or_else(|| {
                NoteUseCaseError::NotFound(format!("initial version for note {note_id} not found"))
            })?;
        Ok(NoteWriteResult {
            note_id,
            created,
            snapshot: NoteSnapshot {
                note,
                version,
                created_by_kind,
            },
        })
    }
}

#[cfg(all(test, feature = "test-support"))]
mod tests {
    use std::sync::Arc;

    use async_trait::async_trait;
    use chrono::{DateTime, FixedOffset, Utc};
    use serde_json::json;
    use tokio::sync::Mutex;

    use super::*;
    use crate::change_history::{Actor, FakeChangeHistory};
    use crate::note::repository::{NoteRepository, NoteRepositoryError};
    use crate::note::types::{
        NewNoteLink, NewNoteVersion, Note, NoteLinkTarget, NoteMetadataUpdate, NoteVersion,
        NoteVersionUpdate,
    };
    use crate::unit_of_work::FakeUnitOfWork;

    const SOURCE_NOTE_ID: Uuid = Uuid::from_u128(1);
    const TARGET_NOTE_ID: Uuid = Uuid::from_u128(2);
    const SOURCE_VERSION_ID: Uuid = Uuid::from_u128(3);
    const TARGET_VERSION_ID: Uuid = Uuid::from_u128(4);
    const NORMALIZED_VERSION_ID: Uuid = Uuid::from_u128(7);

    struct LinkNoteRepository {
        source_note: Note,
        source_version: NoteVersion,
        target: NoteLinkTarget,
        inserted_links: Mutex<Vec<NewNoteLink>>,
    }

    #[async_trait]
    impl NoteRepository for LinkNoteRepository {
        async fn find_note(
            &self,
            _transaction: &UnitOfWorkTransaction,
            note_id: Uuid,
        ) -> Result<Option<Note>, NoteRepositoryError> {
            Ok((note_id == SOURCE_NOTE_ID).then(|| self.source_note.clone()))
        }

        async fn find_note_by_execution_id(
            &self,
            _transaction: &UnitOfWorkTransaction,
            _execution_id: &str,
        ) -> Result<Option<Note>, NoteRepositoryError> {
            Ok(None)
        }

        async fn find_note_kind_requires_approval(
            &self,
            _transaction: &UnitOfWorkTransaction,
            _kind: &str,
        ) -> Result<Option<bool>, NoteRepositoryError> {
            Ok(Some(false))
        }

        async fn find_current_version(
            &self,
            _transaction: &UnitOfWorkTransaction,
            note_id: Uuid,
        ) -> Result<Option<NoteVersion>, NoteRepositoryError> {
            Ok((note_id == SOURCE_NOTE_ID).then(|| self.source_version.clone()))
        }

        async fn find_latest_version(
            &self,
            _transaction: &UnitOfWorkTransaction,
            note_id: Uuid,
        ) -> Result<Option<NoteVersion>, NoteRepositoryError> {
            Ok((note_id == SOURCE_NOTE_ID).then(|| self.source_version.clone()))
        }

        async fn find_latest_pending_versions_by_kind(
            &self,
            _transaction: &UnitOfWorkTransaction,
            _kind: &str,
        ) -> Result<Vec<NoteVersion>, NoteRepositoryError> {
            Ok(Vec::new())
        }

        async fn find_version_by_number(
            &self,
            _transaction: &UnitOfWorkTransaction,
            _note_id: Uuid,
            _version_no: i32,
        ) -> Result<Option<NoteVersion>, NoteRepositoryError> {
            Ok(None)
        }

        async fn supersede_pending_versions_before(
            &self,
            _transaction: &UnitOfWorkTransaction,
            _note_id: Uuid,
            _version_no: i32,
        ) -> Result<Vec<Uuid>, NoteRepositoryError> {
            Ok(Vec::new())
        }

        async fn find_initial_created_by_kind(
            &self,
            _transaction: &UnitOfWorkTransaction,
            note_id: Uuid,
        ) -> Result<Option<String>, NoteRepositoryError> {
            Ok((note_id == SOURCE_NOTE_ID).then(|| "human".into()))
        }

        async fn insert_note(
            &self,
            _transaction: &UnitOfWorkTransaction,
            _note: NewNote,
        ) -> Result<Option<Note>, NoteRepositoryError> {
            Ok(None)
        }

        async fn update_note(
            &self,
            _transaction: &UnitOfWorkTransaction,
            _update: NoteMetadataUpdate,
        ) -> Result<Note, NoteRepositoryError> {
            Ok(self.source_note.clone())
        }

        async fn delete_note(
            &self,
            _transaction: &UnitOfWorkTransaction,
            _note_id: Uuid,
        ) -> Result<bool, NoteRepositoryError> {
            Ok(false)
        }

        async fn insert_version(
            &self,
            _transaction: &UnitOfWorkTransaction,
            version: NewNoteVersion,
        ) -> Result<NoteVersion, NoteRepositoryError> {
            Ok(NoteVersion {
                id: version.id,
                note_id: version.note_id,
                version_no: version.version_no,
                title: version.title,
                body_md: version.body_md,
                frontmatter_json: version.frontmatter_json,
                graphs_json: version.graphs_json,
                status: version.status,
                is_current: version.is_current,
                change_reason: version.change_reason,
                created_by_kind: version.created_by_kind,
                execution_id: version.execution_id,
                created_at: timestamp(),
                reviewed_at: None,
            })
        }

        async fn update_version(
            &self,
            _transaction: &UnitOfWorkTransaction,
            update: NoteVersionUpdate,
        ) -> Result<NoteVersion, NoteRepositoryError> {
            let mut version = self.source_version.clone();
            if version.id != update.id {
                version.id = update.id;
            }
            if let Some(is_current) = update.is_current {
                version.is_current = is_current;
            }
            if let Some(status) = update.status {
                version.status = status;
            }
            if let Some(reviewed_at) = update.reviewed_at {
                version.reviewed_at = Some(reviewed_at);
            }
            Ok(version)
        }

        async fn update_note_timestamp(
            &self,
            _transaction: &UnitOfWorkTransaction,
            _note_id: Uuid,
            _updated_at: DateTime<FixedOffset>,
        ) -> Result<(), NoteRepositoryError> {
            Ok(())
        }

        async fn replace_references(
            &self,
            _transaction: &UnitOfWorkTransaction,
            _note_id: Uuid,
            _references: Vec<(String, String)>,
        ) -> Result<(), NoteRepositoryError> {
            Ok(())
        }

        async fn find_note_link_targets(
            &self,
            _transaction: &UnitOfWorkTransaction,
            note_ids: &[Uuid],
        ) -> Result<Vec<NoteLinkTarget>, NoteRepositoryError> {
            Ok(note_ids
                .contains(&self.target.id)
                .then_some(self.target)
                .into_iter()
                .collect())
        }

        async fn insert_note_links(
            &self,
            _transaction: &UnitOfWorkTransaction,
            links: Vec<NewNoteLink>,
        ) -> Result<(), NoteRepositoryError> {
            self.inserted_links.lock().await.extend(links);
            Ok(())
        }

        async fn copy_note_links(
            &self,
            _transaction: &UnitOfWorkTransaction,
            _from_version_id: Uuid,
            _to_version_id: Uuid,
        ) -> Result<(), NoteRepositoryError> {
            Ok(())
        }
    }

    fn source_note() -> Note {
        Note {
            id: SOURCE_NOTE_ID,
            kind: None,
            trigger: None,
            trigger_label: None,
            created_at: timestamp(),
            updated_at: timestamp(),
            execution_id: None,
        }
    }

    fn source_version() -> NoteVersion {
        NoteVersion {
            id: SOURCE_VERSION_ID,
            note_id: SOURCE_NOTE_ID,
            version_no: 1,
            title: "Previous title".into(),
            body_md: "Previous body".into(),
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

    #[tokio::test]
    async fn write_updates_a_note_and_links_another_note() {
        let repository = Arc::new(LinkNoteRepository {
            source_note: source_note(),
            source_version: source_version(),
            target: NoteLinkTarget {
                id: TARGET_NOTE_ID,
                current_version_id: Some(TARGET_VERSION_ID),
            },
            inserted_links: Mutex::new(Vec::new()),
        });
        let unit_of_work = Arc::new(FakeUnitOfWork::new());
        let use_cases = NoteUseCases::new(
            unit_of_work,
            repository.clone(),
            Arc::new(FakeChangeHistory::new()),
        );

        let mut result = use_cases
            .write(NoteWriteCommand {
                execution_id: None,
                note_id: Some(SOURCE_NOTE_ID),
                title: Some("Updated title".into()),
                body_md: Some(format!("[[note:{TARGET_NOTE_ID}@current]]")),
                frontmatter_json: None,
                graphs_json: None,
                kind: None,
                status: None,
                trigger: None,
                trigger_label: None,
                created_by_kind: "llm".into(),
                change_reason: None,
                actor: Actor::Llm { label: "analyst" },
                change_diff: None,
            })
            .await
            .expect("the note can be updated");

        result.snapshot.version.id = NORMALIZED_VERSION_ID;
        let links = repository
            .inserted_links
            .lock()
            .await
            .iter()
            .map(|link| NewNoteLink {
                from_version_id: NORMALIZED_VERSION_ID,
                ..*link
            })
            .collect::<Vec<_>>();

        assert_eq!(
            (result, links),
            (
                NoteWriteResult {
                    note_id: SOURCE_NOTE_ID,
                    created: false,
                    snapshot: NoteSnapshot {
                        note: source_note(),
                        version: NoteVersion {
                            id: NORMALIZED_VERSION_ID,
                            note_id: SOURCE_NOTE_ID,
                            version_no: 2,
                            title: "Updated title".into(),
                            body_md: format!("[[note:{TARGET_NOTE_ID}@current]]"),
                            frontmatter_json: json!({}),
                            graphs_json: json!([]),
                            status: INITIAL_NOTE_STATUS.into(),
                            is_current: true,
                            change_reason: None,
                            created_by_kind: "llm".into(),
                            execution_id: None,
                            created_at: timestamp(),
                            reviewed_at: None,
                        },
                        created_by_kind: "human".into(),
                    },
                },
                vec![NewNoteLink {
                    from_version_id: NORMALIZED_VERSION_ID,
                    to_note_id: TARGET_NOTE_ID,
                    to_version_id: None,
                }],
            ),
        );
    }
}
