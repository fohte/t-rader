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
    use indoc::indoc;
    use rstest::rstest;
    use serde_json::json;
    use tokio::sync::Mutex;

    use super::*;
    use crate::change_history::{Actor, FakeChangeHistory};
    use crate::note::repository::{NoteRepository, NoteRepositoryError};
    use crate::note::types::{
        NewNoteLink, NewNoteVersion, Note, NoteLinkTarget, NoteMetadataUpdate, NoteVersion,
        NoteVersionUpdate,
    };
    use crate::strategy_task_step_evidence::{
        FakeStrategyTaskStepEvidenceRepository, StrategyTaskStepEvidence,
    };
    use crate::unit_of_work::FakeUnitOfWork;

    const SOURCE_NOTE_ID: Uuid = Uuid::from_u128(1);
    const TARGET_NOTE_ID: Uuid = Uuid::from_u128(2);
    const SOURCE_VERSION_ID: Uuid = Uuid::from_u128(3);
    const TARGET_VERSION_ID: Uuid = Uuid::from_u128(4);
    const EXECUTION_STEP_ID: Uuid = Uuid::from_u128(5);
    const NORMALIZED_VERSION_ID: Uuid = Uuid::from_u128(7);

    struct LinkNoteRepository {
        source_note: Note,
        source_version: NoteVersion,
        latest_version: NoteVersion,
        requires_approval: bool,
        target: NoteLinkTarget,
        inserted_links: Mutex<Vec<NewNoteLink>>,
        inserted_versions: Mutex<Vec<NewNoteVersion>>,
        references: Mutex<Vec<(String, String)>>,
        reference_replacements: Mutex<Vec<Vec<(String, String)>>>,
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
            Ok(Some(self.requires_approval))
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
            Ok((note_id == SOURCE_NOTE_ID).then(|| self.latest_version.clone()))
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
            self.inserted_versions.lock().await.push(version.clone());
            Ok(NoteVersion {
                id: version.id,
                note_id: version.note_id,
                version_no: version.version_no,
                title: version.title,
                body_md: version.body_md,
                frontmatter_json: version.frontmatter_json,
                graphs_json: version.graphs_json,
                resolved_price_references_json: version.resolved_price_references_json,
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
            references: Vec<(String, String)>,
        ) -> Result<(), NoteRepositoryError> {
            *self.references.lock().await = references.clone();
            self.reference_replacements.lock().await.push(references);
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
        source_note_with_kind(None)
    }

    fn source_note_with_kind(kind: Option<String>) -> Note {
        Note {
            id: SOURCE_NOTE_ID,
            kind,
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
            resolved_price_references_json: json!({}),
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

    fn make_note_use_cases(
        source_version: NoteVersion,
        evidence: Vec<StrategyTaskStepEvidence>,
    ) -> (NoteUseCases, Arc<LinkNoteRepository>) {
        make_note_use_cases_with_latest(source_version.clone(), source_version, evidence)
    }

    fn make_note_use_cases_with_latest(
        source_version: NoteVersion,
        latest_version: NoteVersion,
        evidence: Vec<StrategyTaskStepEvidence>,
    ) -> (NoteUseCases, Arc<LinkNoteRepository>) {
        make_note_use_cases_with_options(
            source_version,
            latest_version,
            evidence,
            false,
            None,
            vec![],
        )
    }

    fn make_approval_note_use_cases(
        source_version: NoteVersion,
        references: Vec<(String, String)>,
    ) -> (NoteUseCases, Arc<LinkNoteRepository>) {
        make_note_use_cases_with_options(
            source_version.clone(),
            source_version,
            vec![],
            true,
            Some("sample-kind".into()),
            references,
        )
    }

    fn make_note_use_cases_with_options(
        source_version: NoteVersion,
        latest_version: NoteVersion,
        evidence: Vec<StrategyTaskStepEvidence>,
        requires_approval: bool,
        kind: Option<String>,
        references: Vec<(String, String)>,
    ) -> (NoteUseCases, Arc<LinkNoteRepository>) {
        let repository = Arc::new(LinkNoteRepository {
            source_note: source_note_with_kind(kind),
            source_version,
            latest_version,
            requires_approval,
            target: NoteLinkTarget {
                id: TARGET_NOTE_ID,
                current_version_id: Some(TARGET_VERSION_ID),
            },
            inserted_links: Mutex::new(Vec::new()),
            inserted_versions: Mutex::new(Vec::new()),
            references: Mutex::new(references),
            reference_replacements: Mutex::new(Vec::new()),
        });
        let evidence_repository = FakeStrategyTaskStepEvidenceRepository::new(evidence);
        let use_cases = NoteUseCases::new(
            Arc::new(FakeUnitOfWork::new()),
            repository.clone(),
            Arc::new(FakeChangeHistory::new()),
            Arc::new(evidence_repository),
        );
        (use_cases, repository)
    }

    fn query_data_evidence() -> StrategyTaskStepEvidence {
        StrategyTaskStepEvidence {
            id: Uuid::from_u128(8),
            execution_step_id: EXECUTION_STEP_ID,
            source: "query_data".into(),
            source_ref: "fictional-code".into(),
            observed_at: timestamp(),
            published_at: None,
            effective_at: None,
            snapshot: json!({
                "instrument_id": "fictional-code",
                "bars": [
                    {
                        "timestamp": "2030-01-02T00:00:00Z",
                        "open": 9.0,
                        "high": 11.0,
                        "low": 8.0,
                        "close": 10.0,
                        "volume": 100,
                    },
                    {
                        "timestamp": "2030-01-03T00:00:00Z",
                        "open": 10.0,
                        "high": 13.0,
                        "low": 9.0,
                        "close": 12.0,
                        "volume": 150,
                    },
                ],
            }),
        }
    }

    #[tokio::test]
    async fn write_updates_a_note_and_links_another_note() {
        let (use_cases, repository) = make_note_use_cases(source_version(), vec![]);

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
                            resolved_price_references_json: json!({}),
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

    #[tokio::test]
    async fn write_resolves_price_links_and_keeps_them_in_the_body() {
        let mut source_version = source_version();
        source_version.execution_id = Some(EXECUTION_STEP_ID.to_string());
        let (use_cases, _) = make_note_use_cases(source_version, vec![query_data_evidence()]);
        let body = concat!(
            "[[price:fictional-code@2030-01-02:open]] ",
            "[[price:fictional-code@2030-01-02:high]] ",
            "[[price:fictional-code@2030-01-02:low]] ",
            "[[price:fictional-code@2030-01-02:close]] ",
            "[[price:fictional-code@2030-01-02:volume]] ",
            "[[change:fictional-code@2030-01-02..2030-01-03:close]]",
        );

        let mut result = use_cases
            .write(NoteWriteCommand {
                execution_id: Some(EXECUTION_STEP_ID.to_string()),
                note_id: Some(SOURCE_NOTE_ID),
                title: Some("Updated title".into()),
                body_md: Some(body.into()),
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
            .expect("price links resolve from the execution evidence");
        result.snapshot.version.id = NORMALIZED_VERSION_ID;

        assert_eq!(
            result,
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
                        body_md: body.into(),
                        frontmatter_json: json!({}),
                        graphs_json: json!([]),
                        resolved_price_references_json: json!({
                            "[[price:fictional-code@2030-01-02:open]]": {
                                "value": 9.0,
                                "evidence_id": Uuid::from_u128(8).to_string(),
                            },
                            "[[price:fictional-code@2030-01-02:high]]": {
                                "value": 11.0,
                                "evidence_id": Uuid::from_u128(8).to_string(),
                            },
                            "[[price:fictional-code@2030-01-02:low]]": {
                                "value": 8.0,
                                "evidence_id": Uuid::from_u128(8).to_string(),
                            },
                            "[[price:fictional-code@2030-01-02:close]]": {
                                "value": 10.0,
                                "evidence_id": Uuid::from_u128(8).to_string(),
                            },
                            "[[price:fictional-code@2030-01-02:volume]]": {
                                "value": 100,
                                "evidence_id": Uuid::from_u128(8).to_string(),
                            },
                            "[[change:fictional-code@2030-01-02..2030-01-03:close]]": {
                                "value": 20.0,
                                "evidence_id": Uuid::from_u128(8).to_string(),
                            },
                        }),
                        status: INITIAL_NOTE_STATUS.into(),
                        is_current: true,
                        change_reason: None,
                        created_by_kind: "llm".into(),
                        execution_id: Some(EXECUTION_STEP_ID.to_string()),
                        created_at: timestamp(),
                        reviewed_at: None,
                    },
                    created_by_kind: "human".into(),
                },
            },
        );
    }

    #[tokio::test]
    async fn human_edit_uses_current_version_evidence_without_inheriting_execution_id() {
        let link = "[[price:fictional-code@2030-01-02:close]]";
        let mut current = source_version();
        current.body_md = link.into();
        current.execution_id = Some(EXECUTION_STEP_ID.to_string());
        let mut latest = current.clone();
        latest.id = Uuid::from_u128(12);
        latest.version_no = 2;
        latest.body_md = "pending revision".into();
        latest.execution_id = Some(Uuid::from_u128(13).to_string());
        latest.status = "unread".into();
        latest.is_current = false;
        let (use_cases, repository) =
            make_note_use_cases_with_latest(current, latest, vec![query_data_evidence()]);

        use_cases
            .write(NoteWriteCommand {
                execution_id: None,
                note_id: Some(SOURCE_NOTE_ID),
                title: Some("Human title edit".into()),
                body_md: Some(link.into()),
                frontmatter_json: None,
                graphs_json: None,
                kind: None,
                status: None,
                trigger: None,
                trigger_label: None,
                created_by_kind: "human".into(),
                change_reason: None,
                actor: Actor::Human,
                change_diff: None,
            })
            .await
            .expect("current version evidence resolves the link");

        let mut actual = repository
            .inserted_versions
            .lock()
            .await
            .last()
            .cloned()
            .expect("the human edit creates a version");
        actual.id = Uuid::nil();
        assert_eq!(
            actual,
            NewNoteVersion {
                id: Uuid::nil(),
                note_id: SOURCE_NOTE_ID,
                version_no: 3,
                title: "Human title edit".into(),
                body_md: link.into(),
                frontmatter_json: json!({}),
                graphs_json: json!([]),
                resolved_price_references_json: json!({
                    "[[price:fictional-code@2030-01-02:close]]": {
                        "value": 10.0,
                        "evidence_id": Uuid::from_u128(8).to_string(),
                    },
                }),
                status: "approved".into(),
                is_current: true,
                change_reason: None,
                created_by_kind: "human".into(),
                execution_id: None,
            },
        );
    }

    #[tokio::test]
    async fn human_edit_preserves_resolutions_when_body_is_unchanged() {
        let link = "[[price:fictional-code@2030-01-02:close]]";
        let mut current = source_version();
        current.body_md = link.into();
        current.resolved_price_references_json = json!({
            "[[price:fictional-code@2030-01-02:close]]": {
                "value": 10.0,
                "evidence_id": Uuid::from_u128(8).to_string(),
            },
        });
        let (use_cases, repository) = make_note_use_cases(current, vec![]);

        use_cases
            .write(NoteWriteCommand {
                execution_id: None,
                note_id: Some(SOURCE_NOTE_ID),
                title: Some("Human title edit".into()),
                body_md: Some(link.into()),
                frontmatter_json: None,
                graphs_json: None,
                kind: None,
                status: None,
                trigger: None,
                trigger_label: None,
                created_by_kind: "human".into(),
                change_reason: None,
                actor: Actor::Human,
                change_diff: None,
            })
            .await
            .expect("an unchanged body keeps its saved price resolutions");

        let mut actual = repository
            .inserted_versions
            .lock()
            .await
            .last()
            .cloned()
            .expect("the human edit creates a version");
        actual.id = Uuid::nil();
        assert_eq!(
            actual,
            NewNoteVersion {
                id: Uuid::nil(),
                note_id: SOURCE_NOTE_ID,
                version_no: 2,
                title: "Human title edit".into(),
                body_md: link.into(),
                frontmatter_json: json!({}),
                graphs_json: json!([]),
                resolved_price_references_json: json!({
                    "[[price:fictional-code@2030-01-02:close]]": {
                        "value": 10.0,
                        "evidence_id": Uuid::from_u128(8).to_string(),
                    },
                }),
                status: "approved".into(),
                is_current: true,
                change_reason: None,
                created_by_kind: "human".into(),
                execution_id: None,
            },
        );
    }

    #[tokio::test]
    async fn human_body_edit_recovers_execution_context_from_saved_evidence() {
        let previous_link = "[[price:fictional-code@2030-01-02:close]]";
        let updated_link = "[[price:fictional-code@2030-01-03:close]]";
        let mut current = source_version();
        current.body_md = previous_link.into();
        current.resolved_price_references_json = json!({
            "[[price:fictional-code@2030-01-02:close]]": {
                "value": 10.0,
                "evidence_id": Uuid::from_u128(8).to_string(),
            },
        });
        let (use_cases, repository) = make_note_use_cases(current, vec![query_data_evidence()]);

        use_cases
            .write(NoteWriteCommand {
                execution_id: None,
                note_id: Some(SOURCE_NOTE_ID),
                title: Some("Human body edit".into()),
                body_md: Some(updated_link.into()),
                frontmatter_json: None,
                graphs_json: None,
                kind: None,
                status: None,
                trigger: None,
                trigger_label: None,
                created_by_kind: "human".into(),
                change_reason: None,
                actor: Actor::Human,
                change_diff: None,
            })
            .await
            .expect("saved evidence identifies the original execution");

        let mut actual = repository
            .inserted_versions
            .lock()
            .await
            .last()
            .cloned()
            .expect("the human edit creates a version");
        actual.id = Uuid::nil();
        assert_eq!(
            actual,
            NewNoteVersion {
                id: Uuid::nil(),
                note_id: SOURCE_NOTE_ID,
                version_no: 2,
                title: "Human body edit".into(),
                body_md: updated_link.into(),
                frontmatter_json: json!({}),
                graphs_json: json!([]),
                resolved_price_references_json: json!({
                    "[[price:fictional-code@2030-01-03:close]]": {
                        "value": 12.0,
                        "evidence_id": Uuid::from_u128(8).to_string(),
                    },
                }),
                status: "approved".into(),
                is_current: true,
                change_reason: None,
                created_by_kind: "human".into(),
                execution_id: None,
            },
        );
    }

    #[rstest]
    #[case::missing_execution(
        None,
        "[[price:fictional-code@2030-01-02:close]]",
        "価格参照の解決には実行ステップの query_data が必要です"
    )]
    #[case::missing_instrument(Some(EXECUTION_STEP_ID.to_string()), "[[price:fictional-other@2030-01-02:close]]", "価格参照 [[price:fictional-other@2030-01-02:close]] を実行ステップの query_data から解決できません")]
    #[case::missing_date(Some(EXECUTION_STEP_ID.to_string()), "[[price:fictional-code@2030-01-04:close]]", "価格参照 [[price:fictional-code@2030-01-04:close]] を実行ステップの query_data から解決できません")]
    #[case::missing_change_endpoint(Some(EXECUTION_STEP_ID.to_string()), "[[change:fictional-code@2030-01-02..2030-01-04:close]]", "価格参照 [[change:fictional-code@2030-01-02..2030-01-04:close]] を実行ステップの query_data から解決できません")]
    #[tokio::test]
    async fn write_rejects_unresolved_price_links_before_inserting_a_version(
        #[case] source_execution_id: Option<String>,
        #[case] body: &str,
        #[case] expected_error: &str,
    ) {
        let mut source_version = source_version();
        source_version.execution_id = source_execution_id;
        let (use_cases, repository) =
            make_note_use_cases(source_version, vec![query_data_evidence()]);
        let result = use_cases
            .write(NoteWriteCommand {
                execution_id: None,
                note_id: Some(SOURCE_NOTE_ID),
                title: Some("Updated title".into()),
                body_md: Some(body.into()),
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
            .await;
        let inserted_version_count = repository.inserted_versions.lock().await.len();

        assert_eq!(
            (
                result.err().map(|error| error.to_string()),
                inserted_version_count,
            ),
            (Some(expected_error.into()), 0),
        );
    }

    #[tokio::test]
    async fn write_rejects_missing_graph_references_for_approval_required_notes() {
        let (use_cases, repository) = make_approval_note_use_cases(
            source_version(),
            vec![("stock".into(), "sample-code".into())],
        );
        let result = use_cases
            .write(NoteWriteCommand {
                execution_id: None,
                note_id: Some(SOURCE_NOTE_ID),
                title: Some("Updated title".into()),
                body_md: Some("[[graph:missing-chart]]".into()),
                frontmatter_json: None,
                graphs_json: None,
                kind: None,
                status: None,
                trigger: None,
                trigger_label: None,
                created_by_kind: "llm".into(),
                change_reason: Some("Update graph reference".into()),
                actor: Actor::Llm { label: "analyst" },
                change_diff: None,
            })
            .await
            .map(|_| ());
        let inserted_versions = repository.inserted_versions.lock().await.len();
        let references = repository.references.lock().await.clone();
        let replacements = repository.reference_replacements.lock().await.clone();

        assert_eq!(
            (
                result.err().map(|error| error.to_string()),
                inserted_versions,
                references,
                replacements,
            ),
            (
                Some(indoc! {"
                    ノートのトークンに問題があります:
                    - 本文のトークン \"[[graph:missing-chart]]\": 対応する graphs[].id がありません
                    許可される形式: `[[stock:<id>]]`, `[[indicator:<id>]]`, `[[group:<axis-key>/<group-key>]]`, `[[note:<uuid>]]`, `[[note:<uuid>@current]]`, `[[anno:<id>]]`, `[[price:<id>@<date>:<field>]]`, `[[change:<id>@<start>..<end>:<field>]]`。`[[graph:<id>]]` は graphs[].id に存在し、空行区切りブロック内で単独にしてください。graphs[].nodes[].ref では参照 3 種のみ使用できます。
                "}
                .trim_end()
                .to_string()),
                0,
                vec![("stock".into(), "sample-code".into())],
                vec![],
            ),
        );
    }

    #[tokio::test]
    async fn write_keeps_pending_graph_references_out_of_the_reference_table() {
        let existing_references = vec![("stock".into(), "sample-code".into())];
        let (use_cases, repository) =
            make_approval_note_use_cases(source_version(), existing_references.clone());
        let result = use_cases
            .write(NoteWriteCommand {
                execution_id: None,
                note_id: Some(SOURCE_NOTE_ID),
                title: Some("Updated title".into()),
                body_md: Some("[[graph:sample-chart]]".into()),
                frontmatter_json: None,
                graphs_json: Some(json!([{
                    "id": "sample-chart",
                    "layout": "flow",
                    "nodes": [],
                    "edges": [],
                }])),
                kind: None,
                status: None,
                trigger: None,
                trigger_label: None,
                created_by_kind: "llm".into(),
                change_reason: Some("Update graph reference".into()),
                actor: Actor::Llm { label: "analyst" },
                change_diff: None,
            })
            .await
            .map(|result| result.snapshot.version);
        let mut inserted_versions = repository.inserted_versions.lock().await.clone();
        for version in &mut inserted_versions {
            version.id = Uuid::nil();
        }
        let references = repository.references.lock().await.clone();
        let replacements = repository.reference_replacements.lock().await.clone();

        assert_eq!(
            (
                result
                    .map(|mut version| {
                        version.id = Uuid::nil();
                        version
                    })
                    .map_err(|error| error.to_string()),
                inserted_versions,
                references,
                replacements,
            ),
            (
                Ok(NoteVersion {
                    id: Uuid::nil(),
                    note_id: SOURCE_NOTE_ID,
                    version_no: 2,
                    title: "Updated title".into(),
                    body_md: "[[graph:sample-chart]]".into(),
                    frontmatter_json: json!({}),
                    graphs_json: json!([{
                        "id": "sample-chart",
                        "layout": "flow",
                        "nodes": [],
                        "edges": [],
                    }]),
                    resolved_price_references_json: json!({}),
                    status: INITIAL_NOTE_STATUS.into(),
                    is_current: false,
                    change_reason: Some("Update graph reference".into()),
                    created_by_kind: "llm".into(),
                    execution_id: None,
                    created_at: timestamp(),
                    reviewed_at: None,
                }),
                vec![NewNoteVersion {
                    id: Uuid::nil(),
                    note_id: SOURCE_NOTE_ID,
                    version_no: 2,
                    title: "Updated title".into(),
                    body_md: "[[graph:sample-chart]]".into(),
                    frontmatter_json: json!({}),
                    graphs_json: json!([{
                        "id": "sample-chart",
                        "layout": "flow",
                        "nodes": [],
                        "edges": [],
                    }]),
                    resolved_price_references_json: json!({}),
                    status: INITIAL_NOTE_STATUS.into(),
                    is_current: false,
                    change_reason: Some("Update graph reference".into()),
                    created_by_kind: "llm".into(),
                    execution_id: None,
                }],
                existing_references,
                vec![],
            ),
        );
    }
}
