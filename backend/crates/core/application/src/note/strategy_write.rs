use chrono::Utc;
use serde_json::{Map, json};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

use core_domain::note_body_warning::{
    NoteBodyWarning, scan_note_body_warnings_with_ids, warning_id_group,
};

use crate::note::frontmatter_validation::ensure_frontmatter_tags_are_strings;
use crate::note::types::{
    NewNote, NoteBodyWarningDisposition, NoteSnapshot, NoteVersion, NoteVersionUpdate,
    NoteWriteCommand, NoteWriteResult,
};
use crate::note::use_cases::ensure_frontmatter_object;
use crate::note::version_write::AppendVersionCommand;
use crate::note::widget_blocks::validate_note_widget_blocks;
use crate::note::{INITIAL_NOTE_STATUS, NoteUseCaseError, NoteUseCases};
use crate::unit_of_work::UnitOfWorkTransaction;

const ALLOWED_STATUSES: [&str; 3] = ["approved", "unread", "rejected"];
const ALLOWED_CREATED_BY: [&str; 2] = ["human", "llm"];

impl NoteUseCases {
    pub async fn write(
        &self,
        command: NoteWriteCommand,
    ) -> Result<NoteWriteResult, NoteUseCaseError> {
        self.write_inner(command, None).await
    }

    pub async fn write_with_warning_dispositions(
        &self,
        command: NoteWriteCommand,
        warning_dispositions: Vec<NoteBodyWarningDisposition>,
    ) -> Result<NoteWriteResult, NoteUseCaseError> {
        self.write_inner(command, Some(warning_dispositions)).await
    }

    async fn write_inner(
        &self,
        command: NoteWriteCommand,
        warning_dispositions: Option<Vec<NoteBodyWarningDisposition>>,
    ) -> Result<NoteWriteResult, NoteUseCaseError> {
        if let Some(body_md) = command.body_md.as_deref() {
            validate_note_widget_blocks(body_md)?;
        }

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
        let resolved_market_data = if let Some(body_md) = command.body_md.as_deref() {
            Some(self.resolve_market_data_references(body_md).await)
        } else {
            None
        };
        if let Some(note_id) = command.note_id {
            let transaction = self.unit_of_work.begin().await?;
            let note = self.require_note(&transaction, note_id).await?;
            let result = self
                .update_strategy_note(
                    &transaction,
                    note,
                    command,
                    resolved_market_data,
                    warning_dispositions.as_deref(),
                )
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
                .update_strategy_note(
                    &transaction,
                    note,
                    command,
                    resolved_market_data,
                    warning_dispositions.as_deref(),
                )
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
        validate_body_warning_dispositions(
            command.body_md.as_deref().unwrap_or_default(),
            warning_dispositions.as_deref(),
        )?;

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
                .update_strategy_note(
                    &transaction,
                    note,
                    command,
                    resolved_market_data,
                    warning_dispositions.as_deref(),
                )
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
                    resolved_market_data,
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
        resolved_market_data: Option<Result<Map<String, serde_json::Value>, NoteUseCaseError>>,
        warning_dispositions: Option<&[NoteBodyWarningDisposition]>,
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
        validate_body_warning_dispositions(&body_md, warning_dispositions)?;
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
                    resolved_market_data,
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

fn validate_body_warning_dispositions(
    body_md: &str,
    warning_dispositions: Option<&[NoteBodyWarningDisposition]>,
) -> Result<(), NoteUseCaseError> {
    let Some(warning_dispositions) = warning_dispositions else {
        return Ok(());
    };
    let warnings = scan_note_body_warnings_with_ids(body_md);
    let warning_counts = warnings.iter().fold(HashMap::new(), |mut counts, warning| {
        if let Some(group_id) = warning_id_group(&warning.id) {
            *counts.entry(group_id.to_string()).or_insert(0_usize) += 1;
        }
        counts
    });
    let disposition_ids = warning_dispositions
        .iter()
        .filter(|disposition| !disposition.reason.trim().is_empty())
        .filter_map(|disposition| {
            warning_id_group(&disposition.id)
                .filter(|group_id| warning_counts.contains_key(*group_id))
                .map(|group_id| (group_id.to_string(), disposition.id.clone()))
        })
        .fold(
            HashMap::<String, HashSet<String>>::new(),
            |mut groups, (group_id, id)| {
                groups.entry(group_id).or_default().insert(id);
                groups
            },
        );
    let unresolved_warnings = warnings
        .into_iter()
        .filter(|warning| {
            let Some(group_id) = warning_id_group(&warning.id) else {
                return true;
            };
            let Some(group_dispositions) = disposition_ids.get(group_id) else {
                return true;
            };
            if group_dispositions.contains(&warning.id) {
                return false;
            }
            // 同一警告の先行出現が削除されると番号がずれるため、同じ警告群の理由数で照合する。
            group_dispositions.len() < warning_counts[group_id]
        })
        .collect::<Vec<NoteBodyWarning>>();
    if unresolved_warnings.is_empty() {
        Ok(())
    } else {
        Err(NoteUseCaseError::UnresolvedBodyWarnings {
            warnings: unresolved_warnings,
        })
    }
}

#[cfg(all(test, feature = "test-support"))]
mod tests {
    use std::sync::Arc;

    use async_trait::async_trait;
    use chrono::{DateTime, FixedOffset, NaiveDate, Utc};
    use core_domain::bar::{Bar, Timeframe};
    use core_domain::valuation::Valuation;
    use indoc::indoc;
    use rstest::rstest;
    use rust_decimal::Decimal;
    use serde_json::json;
    use tokio::sync::Mutex;

    use super::*;
    use crate::bars::{BarsUseCases, DailyBarAdjustmentFactor, FakeBarsRepository};
    use crate::change_history::{Actor, FakeChangeHistory};
    use crate::custom_indicator_evaluation::{
        CustomIndicatorEvaluation, CustomIndicatorEvaluationUseCases,
        FakeCustomIndicatorEvaluationRepository,
    };
    use crate::indicator_observation::{
        FakeIndicatorObservationRepository, IndicatorObservationUseCases,
    };
    use crate::note::repository::{NoteRepository, NoteRepositoryError};
    use crate::note::types::{
        NewNoteLink, NewNoteVersion, Note, NoteLinkTarget, NoteMetadataUpdate, NoteVersion,
        NoteVersionUpdate, UpdateNoteCommand,
    };
    use crate::unit_of_work::FakeUnitOfWork;
    use crate::valuation::{FakeValuationRepository, ValuationUseCases};

    const SOURCE_NOTE_ID: Uuid = Uuid::from_u128(1);
    const TARGET_NOTE_ID: Uuid = Uuid::from_u128(2);
    const SOURCE_VERSION_ID: Uuid = Uuid::from_u128(3);
    const TARGET_VERSION_ID: Uuid = Uuid::from_u128(4);
    const EXECUTION_STEP_ID: Uuid = Uuid::from_u128(5);
    const NORMALIZED_VERSION_ID: Uuid = Uuid::from_u128(7);
    const EVALUATION_ID: Uuid = Uuid::from_u128(0x101);

    fn custom_indicator_evaluation_use_cases(
        evaluations: impl IntoIterator<Item = CustomIndicatorEvaluation>,
    ) -> CustomIndicatorEvaluationUseCases {
        let repository = Arc::new(FakeCustomIndicatorEvaluationRepository::new());
        for evaluation in evaluations {
            repository.seed(evaluation);
        }
        CustomIndicatorEvaluationUseCases::new(repository)
    }

    fn custom_indicator_evaluation(output: serde_json::Value) -> CustomIndicatorEvaluation {
        CustomIndicatorEvaluation {
            id: EVALUATION_ID,
            indicator_id: Uuid::from_u128(0x102),
            indicator_name: "demo-indicator".into(),
            code: "return 1".into(),
            args: json!({}),
            output,
            evaluated_at: Utc::now().fixed_offset(),
        }
    }

    struct LinkNoteRepository {
        source_note: Note,
        source_version: NoteVersion,
        latest_version: NoteVersion,
        requires_approval: bool,
        target: NoteLinkTarget,
        inserted_links: Mutex<Vec<NewNoteLink>>,
        inserted_versions: Mutex<Vec<NewNoteVersion>>,
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

    fn write_body_command(body_md: &str) -> NoteWriteCommand {
        NoteWriteCommand {
            execution_id: None,
            note_id: Some(SOURCE_NOTE_ID),
            title: Some("Updated title".into()),
            body_md: Some(body_md.into()),
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
        }
    }

    fn make_note_use_cases(source_version: NoteVersion) -> (NoteUseCases, Arc<LinkNoteRepository>) {
        make_note_use_cases_with_latest(source_version.clone(), source_version)
    }

    fn make_note_use_cases_with_latest(
        source_version: NoteVersion,
        latest_version: NoteVersion,
    ) -> (NoteUseCases, Arc<LinkNoteRepository>) {
        make_note_use_cases_with_options(source_version, latest_version, false)
    }

    fn make_approval_note_use_cases(
        source_version: NoteVersion,
    ) -> (NoteUseCases, Arc<LinkNoteRepository>) {
        make_note_use_cases_with_options(source_version.clone(), source_version, true)
    }

    fn make_note_use_cases_with_options(
        source_version: NoteVersion,
        latest_version: NoteVersion,
        requires_approval: bool,
    ) -> (NoteUseCases, Arc<LinkNoteRepository>) {
        let bars_repository = Arc::new(FakeBarsRepository::new());
        let valuations = ValuationUseCases::new(
            Arc::new(FakeValuationRepository::default()),
            bars_repository.clone(),
        );
        make_note_use_cases_with_services(
            source_version,
            latest_version,
            requires_approval,
            bars_repository,
            valuations,
            IndicatorObservationUseCases::new(Arc::new(FakeIndicatorObservationRepository::new())),
        )
    }

    fn make_note_use_cases_with_bars(
        source_version: NoteVersion,
        bars_repository: Arc<FakeBarsRepository>,
    ) -> (NoteUseCases, Arc<LinkNoteRepository>) {
        make_note_use_cases_with_services(
            source_version.clone(),
            source_version,
            false,
            bars_repository.clone(),
            ValuationUseCases::new(
                Arc::new(FakeValuationRepository::default()),
                bars_repository,
            ),
            IndicatorObservationUseCases::new(Arc::new(FakeIndicatorObservationRepository::new())),
        )
    }

    fn make_note_use_cases_with_services(
        source_version: NoteVersion,
        latest_version: NoteVersion,
        requires_approval: bool,
        bars_repository: Arc<FakeBarsRepository>,
        valuations: ValuationUseCases,
        indicator_observations: IndicatorObservationUseCases,
    ) -> (NoteUseCases, Arc<LinkNoteRepository>) {
        make_note_use_cases_with_all_services(
            source_version,
            latest_version,
            requires_approval,
            bars_repository,
            valuations,
            indicator_observations,
            custom_indicator_evaluation_use_cases([]),
        )
    }

    fn make_note_use_cases_with_all_services(
        source_version: NoteVersion,
        latest_version: NoteVersion,
        requires_approval: bool,
        bars_repository: Arc<FakeBarsRepository>,
        valuations: ValuationUseCases,
        indicator_observations: IndicatorObservationUseCases,
        custom_indicator_evaluations: CustomIndicatorEvaluationUseCases,
    ) -> (NoteUseCases, Arc<LinkNoteRepository>) {
        let repository = Arc::new(LinkNoteRepository {
            source_note: source_note_with_kind(requires_approval.then(|| "sample-kind".into())),
            source_version,
            latest_version,
            requires_approval,
            target: NoteLinkTarget {
                id: TARGET_NOTE_ID,
                current_version_id: Some(TARGET_VERSION_ID),
            },
            inserted_links: Mutex::new(Vec::new()),
            inserted_versions: Mutex::new(Vec::new()),
            reference_replacements: Mutex::new(Vec::new()),
        });
        let unit_of_work = Arc::new(FakeUnitOfWork::new());
        let use_cases = NoteUseCases::new(
            unit_of_work.clone(),
            repository.clone(),
            Arc::new(FakeChangeHistory::new()),
            BarsUseCases::new(unit_of_work, bars_repository),
            valuations,
            indicator_observations,
            custom_indicator_evaluations,
        );
        (use_cases, repository)
    }

    #[expect(clippy::expect_used, reason = "テスト用の固定日時を構築するため")]
    fn daily_bar(
        instrument_id: &str,
        timestamp: &str,
        open: i64,
        high: i64,
        low: i64,
        close: i64,
        volume: i64,
    ) -> Bar {
        Bar {
            instrument_id: instrument_id.into(),
            timeframe: Timeframe::Daily,
            timestamp: DateTime::parse_from_rfc3339(timestamp)
                .expect("valid bar timestamp")
                .with_timezone(&Utc),
            open: Decimal::from(open),
            high: Decimal::from(high),
            low: Decimal::from(low),
            close: Decimal::from(close),
            volume,
            adjustment_factor: Decimal::ONE,
        }
    }

    fn fictional_code_daily_bars() -> Vec<Bar> {
        vec![
            daily_bar("fictional-code", "2030-01-02T00:00:00Z", 9, 11, 8, 10, 100),
            daily_bar("fictional-code", "2030-01-03T00:00:00Z", 10, 13, 9, 12, 150),
        ]
    }

    fn expected_write_result(
        title: &str,
        body: &str,
        resolved_price_references_json: serde_json::Value,
    ) -> NoteWriteResult {
        NoteWriteResult {
            note_id: SOURCE_NOTE_ID,
            created: false,
            snapshot: NoteSnapshot {
                note: source_note(),
                version: NoteVersion {
                    id: NORMALIZED_VERSION_ID,
                    note_id: SOURCE_NOTE_ID,
                    version_no: 2,
                    title: title.into(),
                    body_md: body.into(),
                    frontmatter_json: json!({}),
                    graphs_json: json!([]),
                    resolved_price_references_json,
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
        }
    }

    #[tokio::test]
    async fn write_updates_a_note_and_links_another_note() {
        let (use_cases, repository) = make_note_use_cases(source_version());

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
        let bars_repository = Arc::new(FakeBarsRepository::new());
        let mut bars = fictional_code_daily_bars();
        bars.push(daily_bar(
            "US:FICTIONAL-A",
            "2030-01-02T04:00:00Z",
            13,
            16,
            12,
            15,
            200,
        ));
        bars_repository.seed_bars(bars).await;
        let (use_cases, _) = make_note_use_cases_with_bars(source_version(), bars_repository);
        let body = concat!(
            "[[price:fictional-code@2030-01-02:open]] ",
            "[[price:fictional-code@2030-01-02:high]] ",
            "[[price:fictional-code@2030-01-02:low]] ",
            "[[price:fictional-code@2030-01-02:close]] ",
            "[[price:fictional-code@2030-01-02:volume]] ",
            "[[price:US:FICTIONAL-A@2030-01-02:close]]",
        );

        let mut result = use_cases
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
            .await
            .expect("price references resolve from daily bars without an execution id");

        result.snapshot.version.id = NORMALIZED_VERSION_ID;
        assert_eq!(
            result,
            expected_write_result(
                "Updated title",
                body,
                json!({
                    "[[price:fictional-code@2030-01-02:open]]": { "value": 9 },
                    "[[price:fictional-code@2030-01-02:high]]": { "value": 11 },
                    "[[price:fictional-code@2030-01-02:low]]": { "value": 8 },
                    "[[price:fictional-code@2030-01-02:close]]": { "value": 10 },
                    "[[price:fictional-code@2030-01-02:volume]]": { "value": 100 },
                    "[[price:US:FICTIONAL-A@2030-01-02:close]]": { "value": 15 },
                }),
            ),
        );
    }

    #[tokio::test]
    async fn write_resolves_change_links_from_daily_bars() {
        let bars_repository = Arc::new(FakeBarsRepository::new());
        bars_repository.seed_bars(fictional_code_daily_bars()).await;
        let (use_cases, _) = make_note_use_cases_with_bars(source_version(), bars_repository);
        let reference = "[[change:fictional-code@2030-01-02..2030-01-03:close]]";
        let mut result = use_cases
            .write(write_body_command(reference))
            .await
            .expect("change references resolve from daily bars");

        result.snapshot.version.id = NORMALIZED_VERSION_ID;
        assert_eq!(
            result,
            expected_write_result(
                "Updated title",
                reference,
                json!({
                    "[[change:fictional-code@2030-01-02..2030-01-03:close]]": {
                        "value": 20,
                    },
                }),
            ),
        );
    }

    #[tokio::test]
    async fn write_resolves_valuation_and_indicator_values_without_query_data() {
        let date = "2030-01-02".parse::<NaiveDate>().expect("valid date");
        let valuation_repository = Arc::new(FakeValuationRepository::default());
        valuation_repository
            .valuations
            .lock()
            .expect("lock")
            .push(Valuation {
                code: "fictional-code".into(),
                date,
                eps: Some(Decimal::new(10, 0)),
                fwd_eps: Some(Decimal::new(11, 0)),
                bps: Some(Decimal::new(12, 0)),
                roe: Some(Decimal::new(15, 2)),
                fwd_roe: Some(Decimal::new(18, 2)),
                per: Some(Decimal::new(205, 1)),
                fwd_per: Some(Decimal::new(195, 1)),
                pbr: Some(Decimal::new(12, 1)),
                mkt_cap: Some(Decimal::new(98765, 2)),
            });
        let bars_repository = Arc::new(FakeBarsRepository::new());
        bars_repository
            .seed_daily_adjustment_factors(
                "fictional-code",
                vec![
                    DailyBarAdjustmentFactor {
                        date: "2030-01-01".parse().expect("valid date"),
                        factor: Decimal::new(10, 0),
                    },
                    DailyBarAdjustmentFactor {
                        date: "2030-01-03".parse().expect("valid date"),
                        factor: Decimal::new(2, 0),
                    },
                    DailyBarAdjustmentFactor {
                        date: "2030-01-04".parse().expect("valid date"),
                        factor: Decimal::new(3, 0),
                    },
                ],
            )
            .await;
        let indicator_repository = Arc::new(FakeIndicatorObservationRepository::new());
        indicator_repository
            .seed_observation(
                "demo-indicator",
                "2030-01-01".parse().expect("valid date"),
                Decimal::new(1, 0),
            )
            .await;
        indicator_repository
            .seed_observation("demo-indicator", date, Decimal::new(150123, 3))
            .await;
        let valuations =
            ValuationUseCases::new(valuation_repository.clone(), bars_repository.clone());
        let indicator_observations = IndicatorObservationUseCases::new(indicator_repository);
        let (use_cases, _) = make_note_use_cases_with_all_services(
            source_version(),
            source_version(),
            false,
            bars_repository.clone(),
            valuations,
            indicator_observations,
            custom_indicator_evaluation_use_cases([custom_indicator_evaluation(
                json!({ "result": 256.75 }),
            )]),
        );
        let body = concat!(
            "[[valuation:fictional-code@2030-01-02:per]] ",
            "[[valuation:fictional-code@2030-01-02:fwd_per]] ",
            "[[valuation:fictional-code@2030-01-02:pbr]] ",
            "[[valuation:fictional-code@2030-01-02:roe]] ",
            "[[valuation:fictional-code@2030-01-02:fwd_roe]] ",
            "[[valuation:fictional-code@2030-01-02:mkt_cap]] ",
            "[[valuation:fictional-code@2030-01-02:eps]] ",
            "[[valuation:fictional-code@2030-01-02:fwd_eps]] ",
            "[[valuation:fictional-code@2030-01-02:bps]] ",
            "[[indicator:demo-indicator@2030-01-02]] ",
            "[[eval:00000000-0000-0000-0000-000000000101:result]]",
        );

        let result = use_cases
            .write(write_body_command(body))
            .await
            .expect("date-based values resolve without query_data");

        let resolved = result.snapshot.version.resolved_price_references_json;
        let valuation_queries = valuation_repository
            .date_range_queries
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        let adjustment_factor_queries = bars_repository
            .daily_adjustment_factor_queries
            .lock()
            .await
            .clone();
        assert_eq!(
            (resolved, valuation_queries, adjustment_factor_queries),
            (
                json!({
                    "[[valuation:fictional-code@2030-01-02:per]]": { "value": 20.5 },
                    "[[valuation:fictional-code@2030-01-02:fwd_per]]": { "value": 19.5 },
                    "[[valuation:fictional-code@2030-01-02:pbr]]": { "value": 1.2 },
                    "[[valuation:fictional-code@2030-01-02:roe]]": { "value": 0.15 },
                    "[[valuation:fictional-code@2030-01-02:fwd_roe]]": { "value": 0.18 },
                    "[[valuation:fictional-code@2030-01-02:mkt_cap]]": { "value": 987.65 },
                    "[[valuation:fictional-code@2030-01-02:eps]]": { "value": 60 },
                    "[[valuation:fictional-code@2030-01-02:fwd_eps]]": { "value": 66 },
                    "[[valuation:fictional-code@2030-01-02:bps]]": { "value": 72 },
                    "[[indicator:demo-indicator@2030-01-02]]": { "value": 150.123 },
                    "[[eval:00000000-0000-0000-0000-000000000101:result]]": { "value": 256.75 },
                }),
                vec![("fictional-code".into(), date, date)],
                vec![("fictional-code".into(), date)],
            ),
        );
    }

    #[rstest]
    #[case::missing_evaluation(None, "評価記録が見つかりません")]
    #[case::output_is_not_an_object(Some(json!(256.75)), "出力名を解決できません")]
    #[case::output_name_is_missing(
        Some(json!({ "other": 256.75 })),
        "出力名を解決できません"
    )]
    #[case::output_is_a_string(
        Some(json!({ "result": "256.75" })),
        "出力は数値ではありません"
    )]
    #[case::output_is_an_array(
        Some(json!({ "result": [256.75] })),
        "出力は数値ではありません"
    )]
    #[tokio::test]
    async fn write_rejects_unresolvable_custom_indicator_evaluations(
        #[case] output: Option<serde_json::Value>,
        #[case] reason: &str,
    ) {
        let evaluation_use_cases = match output {
            Some(output) => {
                custom_indicator_evaluation_use_cases([custom_indicator_evaluation(output)])
            }
            None => custom_indicator_evaluation_use_cases([]),
        };
        let bars_repository = Arc::new(FakeBarsRepository::new());
        let (use_cases, _) = make_note_use_cases_with_all_services(
            source_version(),
            source_version(),
            false,
            bars_repository.clone(),
            ValuationUseCases::new(
                Arc::new(FakeValuationRepository::default()),
                bars_repository,
            ),
            IndicatorObservationUseCases::new(Arc::new(FakeIndicatorObservationRepository::new())),
            evaluation_use_cases,
        );
        let token = "[[eval:00000000-0000-0000-0000-000000000101:result]]";

        let result = use_cases.write(write_body_command(token)).await;

        assert_eq!(
            result.map_err(|error| error.to_string()),
            Err(format!("eval 参照 {token} の{reason}")),
        );
    }

    #[rstest]
    #[case::missing_valuation_row(
        "[[valuation:fictional-code@2030-01-02:per]]",
        "valuation 参照 [[valuation:fictional-code@2030-01-02:per]] を指定日のデータから解決できません"
    )]
    #[case::missing_indicator_observation(
        "[[indicator:demo-indicator@2030-01-02]]",
        "indicator 参照 [[indicator:demo-indicator@2030-01-02]] を指定日のデータから解決できません"
    )]
    #[tokio::test]
    async fn write_rejects_missing_valuation_and_indicator_rows(
        #[case] token: &str,
        #[case] expected_error: &str,
    ) {
        let (use_cases, _) = make_note_use_cases(source_version());

        let result = use_cases.write(write_body_command(token)).await;

        assert_eq!(
            result.map_err(|error| error.to_string()),
            Err(expected_error.to_string()),
        );
    }

    #[tokio::test]
    async fn write_rejects_missing_valuation_field_values() {
        let valuation = Valuation {
            code: "fictional-code".into(),
            date: "2030-01-02".parse().expect("valid date"),
            eps: None,
            fwd_eps: None,
            bps: None,
            roe: None,
            fwd_roe: None,
            per: Some(Decimal::new(1, 0)),
            fwd_per: None,
            pbr: None,
            mkt_cap: None,
        };
        let valuation_repository = Arc::new(FakeValuationRepository::default());
        valuation_repository
            .valuations
            .lock()
            .expect("lock")
            .push(valuation);
        let bars_repository = Arc::new(FakeBarsRepository::new());
        let valuations = ValuationUseCases::new(valuation_repository, bars_repository.clone());
        let (use_cases, _) = make_note_use_cases_with_services(
            source_version(),
            source_version(),
            false,
            bars_repository,
            valuations,
            IndicatorObservationUseCases::new(Arc::new(FakeIndicatorObservationRepository::new())),
        );

        let result = use_cases
            .write(write_body_command(
                "[[valuation:fictional-code@2030-01-02:eps]]",
            ))
            .await;

        assert_eq!(
            result.map_err(|error| error.to_string()),
            Err("valuation 参照 [[valuation:fictional-code@2030-01-02:eps]] の項目は記録されていません".into()),
        );
    }

    #[rstest]
    #[case::extra_field(r#"{"value": 10, "metadata": "previous value"}"#)]
    #[case::null_value(r#"{"value": null}"#)]
    #[tokio::test]
    async fn human_edit_re_resolves_entries_without_a_non_null_value_only(
        #[case] saved_entry: &str,
    ) {
        let link = "[[price:fictional-code@2030-01-02:close]]";
        let mut current = source_version();
        current.body_md = link.into();
        current.execution_id = Some(EXECUTION_STEP_ID.to_string());
        current.resolved_price_references_json = json!({
            "[[price:fictional-code@2030-01-02:close]]":
                serde_json::from_str::<serde_json::Value>(saved_entry).expect("valid fixture"),
        });
        let mut latest = current.clone();
        latest.id = Uuid::from_u128(12);
        latest.version_no = 2;
        latest.body_md = "pending revision".into();
        latest.execution_id = Some(Uuid::from_u128(13).to_string());
        latest.status = "unread".into();
        latest.is_current = false;
        let bars_repository = Arc::new(FakeBarsRepository::new());
        bars_repository
            .seed_bars(vec![daily_bar(
                "fictional-code",
                "2030-01-02T00:00:00Z",
                9,
                11,
                8,
                12,
                100,
            )])
            .await;
        let (use_cases, repository) = make_note_use_cases_with_services(
            current,
            latest,
            false,
            bars_repository.clone(),
            ValuationUseCases::new(
                Arc::new(FakeValuationRepository::default()),
                bars_repository,
            ),
            IndicatorObservationUseCases::new(Arc::new(FakeIndicatorObservationRepository::new())),
        );

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
            .expect("current version resolves the link from daily bars");

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
                    "[[price:fictional-code@2030-01-02:close]]": { "value": 12 },
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
    async fn human_edit_keeps_valid_resolutions_when_the_same_body_is_provided() {
        let link = "[[price:fictional-code@2030-01-02:close]]";
        let mut current = source_version();
        current.body_md = link.into();
        current.execution_id = Some(EXECUTION_STEP_ID.to_string());
        current.resolved_price_references_json = json!({
            "[[price:fictional-code@2030-01-02:close]]": { "value": 10 },
        });
        let (use_cases, repository) = make_note_use_cases(current);

        use_cases
            .write(NoteWriteCommand {
                execution_id: Some(EXECUTION_STEP_ID.to_string()),
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
            .expect("a saved value remains stable when the same body is provided without its bar");

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
                    "[[price:fictional-code@2030-01-02:close]]": { "value": 10 },
                }),
                status: "approved".into(),
                is_current: true,
                change_reason: None,
                created_by_kind: "human".into(),
                execution_id: Some(EXECUTION_STEP_ID.to_string()),
            },
        );
    }

    #[rstest]
    #[case::extra_field(r#"{"value": 10, "metadata": "previous value"}"#)]
    #[case::null_value(r#"{"value": null}"#)]
    #[case::legacy_evidence_id(
        r#"{"value": 10, "evidence_id": "00000000-0000-0000-0000-000000000008"}"#
    )]
    #[tokio::test]
    async fn human_title_edit_preserves_invalid_saved_values_when_body_is_omitted(
        #[case] saved_entry: &str,
    ) {
        let link = "[[price:fictional-code@2030-01-02:close]]";
        let mut current = source_version();
        current.body_md = link.into();
        current.resolved_price_references_json = json!({
            "[[price:fictional-code@2030-01-02:close]]":
                serde_json::from_str::<serde_json::Value>(saved_entry).expect("valid fixture"),
        });
        let previous_resolutions = current.resolved_price_references_json.clone();
        let (use_cases, repository) = make_note_use_cases(current);

        use_cases
            .update(
                SOURCE_NOTE_ID,
                UpdateNoteCommand {
                    title: Some("Human title edit".into()),
                    body_md: None,
                    frontmatter_json: None,
                    kind: None,
                    trigger: None,
                    trigger_label: None,
                },
            )
            .await
            .expect("the unchanged body keeps its saved resolutions when the body is omitted");

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
                resolved_price_references_json: previous_resolutions,
                status: "approved".into(),
                is_current: true,
                change_reason: None,
                created_by_kind: "human".into(),
                execution_id: None,
            },
        );
    }

    #[tokio::test]
    async fn title_only_write_keeps_resolutions_for_the_latest_pending_body() {
        let current_link = "[[price:fictional-code@2030-01-02:close]]";
        let pending_link = "[[price:fictional-code@2030-01-03:close]]";
        let mut current = source_version();
        current.body_md = current_link.into();
        current.resolved_price_references_json = json!({
            "[[price:fictional-code@2030-01-02:close]]": { "value": 10 },
        });
        let mut latest = current.clone();
        latest.id = Uuid::from_u128(12);
        latest.version_no = 2;
        latest.body_md = pending_link.into();
        latest.resolved_price_references_json = json!({
            "[[price:fictional-code@2030-01-03:close]]": { "value": 20 },
        });
        latest.status = "unread".into();
        latest.is_current = false;
        let (use_cases, repository) = make_note_use_cases_with_options(current, latest, true);

        use_cases
            .write(NoteWriteCommand {
                execution_id: None,
                note_id: Some(SOURCE_NOTE_ID),
                title: Some("Updated title".into()),
                body_md: None,
                frontmatter_json: None,
                graphs_json: None,
                kind: None,
                status: None,
                trigger: None,
                trigger_label: None,
                created_by_kind: "llm".into(),
                change_reason: Some("Update title".into()),
                actor: Actor::Llm { label: "analyst" },
                change_diff: None,
            })
            .await
            .expect("a title-only write preserves values for the latest pending body");

        let mut actual = repository
            .inserted_versions
            .lock()
            .await
            .last()
            .cloned()
            .expect("the write creates a pending version");
        actual.id = Uuid::nil();
        assert_eq!(
            actual,
            NewNoteVersion {
                id: Uuid::nil(),
                note_id: SOURCE_NOTE_ID,
                version_no: 3,
                title: "Updated title".into(),
                body_md: pending_link.into(),
                frontmatter_json: json!({}),
                graphs_json: json!([]),
                resolved_price_references_json: json!({
                    "[[price:fictional-code@2030-01-03:close]]": { "value": 20 },
                }),
                status: INITIAL_NOTE_STATUS.into(),
                is_current: false,
                change_reason: Some("Update title".into()),
                created_by_kind: "llm".into(),
                execution_id: None,
            },
        );
    }

    #[tokio::test]
    async fn human_edit_preserves_resolutions_when_body_is_unchanged() {
        let body = concat!(
            "[[valuation:fictional-code@2030-01-02:per]] ",
            "[[indicator:demo-indicator@2030-01-02]] ",
            "[[price:fictional-code@2030-01-02:close]] ",
            "[[change:fictional-code@2030-01-02..2030-01-03:close]] ",
            "[[eval:00000000-0000-0000-0000-000000000101:result]]",
        );
        let mut current = source_version();
        current.body_md = body.into();
        current.execution_id = Some(EXECUTION_STEP_ID.to_string());
        current.resolved_price_references_json = json!({
            "[[valuation:fictional-code@2030-01-02:per]]": { "value": 20.5 },
            "[[indicator:demo-indicator@2030-01-02]]": { "value": 150.123 },
            "[[price:fictional-code@2030-01-02:close]]": { "value": 10 },
            "[[change:fictional-code@2030-01-02..2030-01-03:close]]": { "value": 20 },
            "[[eval:00000000-0000-0000-0000-000000000101:result]]": { "value": 256.75 },
        });
        let valuation_repository = Arc::new(FakeValuationRepository::default());
        let indicator_repository = Arc::new(FakeIndicatorObservationRepository::new());
        let bars_repository = Arc::new(FakeBarsRepository::new());
        let (use_cases, repository) = make_note_use_cases_with_services(
            current.clone(),
            current,
            false,
            bars_repository.clone(),
            ValuationUseCases::new(valuation_repository.clone(), bars_repository),
            IndicatorObservationUseCases::new(indicator_repository.clone()),
        );

        use_cases
            .write(NoteWriteCommand {
                execution_id: Some(EXECUTION_STEP_ID.to_string()),
                note_id: Some(SOURCE_NOTE_ID),
                title: Some("Human title edit".into()),
                body_md: None,
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
        let valuation_queries = valuation_repository
            .date_range_queries
            .lock()
            .expect("lock")
            .clone();
        let indicator_queries = indicator_repository
            .read_queries
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        assert_eq!(
            (actual, valuation_queries, indicator_queries),
            (
                NewNoteVersion {
                    id: Uuid::nil(),
                    note_id: SOURCE_NOTE_ID,
                    version_no: 2,
                    title: "Human title edit".into(),
                    body_md: body.into(),
                    frontmatter_json: json!({}),
                    graphs_json: json!([]),
                    resolved_price_references_json: json!({
                        "[[valuation:fictional-code@2030-01-02:per]]": { "value": 20.5 },
                        "[[indicator:demo-indicator@2030-01-02]]": { "value": 150.123 },
                        "[[price:fictional-code@2030-01-02:close]]": { "value": 10 },
                        "[[change:fictional-code@2030-01-02..2030-01-03:close]]": { "value": 20 },
                        "[[eval:00000000-0000-0000-0000-000000000101:result]]": { "value": 256.75 },
                    }),
                    status: "approved".into(),
                    is_current: true,
                    change_reason: None,
                    created_by_kind: "human".into(),
                    execution_id: Some(EXECUTION_STEP_ID.to_string()),
                },
                vec![],
                vec![],
            ),
        );
    }

    #[tokio::test]
    async fn human_body_edit_resolves_new_price_reference_from_bars() {
        let previous_link = "[[price:fictional-code@2030-01-02:close]]";
        let updated_link = "[[price:fictional-code@2030-01-03:close]]";
        let mut current = source_version();
        current.body_md = previous_link.into();
        current.resolved_price_references_json = json!({
            "[[price:fictional-code@2030-01-02:close]]": { "value": 10 },
        });
        let bars_repository = Arc::new(FakeBarsRepository::new());
        bars_repository.seed_bars(fictional_code_daily_bars()).await;
        let (use_cases, repository) = make_note_use_cases_with_bars(current, bars_repository);

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
            .expect("a changed body resolves its new date from daily bars");

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
                    "[[price:fictional-code@2030-01-03:close]]": { "value": 12 },
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
    #[case::missing_instrument(
        "[[price:fictional-other@2030-01-02:close]]",
        "価格参照 [[price:fictional-other@2030-01-02:close]] を指定日の bars から解決できません"
    )]
    #[case::missing_date(
        "[[price:fictional-code@2030-01-04:close]]",
        "価格参照 [[price:fictional-code@2030-01-04:close]] を指定日の bars から解決できません"
    )]
    #[case::missing_change_endpoint(
        "[[change:fictional-code@2030-01-02..2030-01-04:close]]",
        "価格参照 [[change:fictional-code@2030-01-02..2030-01-04:close]] を指定日の bars から解決できません"
    )]
    #[tokio::test]
    async fn write_rejects_unresolved_price_links_before_inserting_a_version(
        #[case] body: &str,
        #[case] expected_error: &str,
    ) {
        let bars_repository = Arc::new(FakeBarsRepository::new());
        bars_repository.seed_bars(fictional_code_daily_bars()).await;
        let (use_cases, repository) =
            make_note_use_cases_with_bars(source_version(), bars_repository);
        let result = use_cases.write(write_body_command(body)).await;
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
    async fn gated_write_rejects_body_warnings_without_saving() {
        let body = "close 45,678円";
        let (use_cases, repository) = make_note_use_cases(source_version());
        let result = use_cases
            .write_with_warning_dispositions(write_body_command(body), vec![])
            .await;
        assert_rejected_without_saving(result, &repository, scan_note_body_warnings_with_ids(body))
            .await;
    }

    #[tokio::test]
    async fn gated_write_checks_the_existing_body_when_body_is_omitted() {
        let body = "close 45,678円";
        let mut current_version = source_version();
        current_version.body_md = body.into();
        let (use_cases, repository) = make_note_use_cases(current_version);
        let mut command = write_body_command("ignored replacement");
        command.body_md = None;
        let result = use_cases
            .write_with_warning_dispositions(command, vec![])
            .await;
        assert_rejected_without_saving(result, &repository, scan_note_body_warnings_with_ids(body))
            .await;
    }

    #[tokio::test]
    async fn gated_write_accepts_a_reason_for_each_warning() {
        let body = "close 45,678円 and -3.5%";
        let (use_cases, repository) = make_note_use_cases(source_version());
        let mut multiline_reason = String::from("分析上の仮定として記載");
        multiline_reason.push('\r');
        multiline_reason.push('\n');
        multiline_reason.push_str("補足理由");
        let warning_dispositions = scan_note_body_warnings_with_ids(body)
            .into_iter()
            .map(|warning| NoteBodyWarningDisposition {
                id: warning.id,
                reason: multiline_reason.clone(),
            })
            .collect();
        let result = use_cases
            .write_with_warning_dispositions(write_body_command(body), warning_dispositions)
            .await;
        let inserted_version_count = repository.inserted_versions.lock().await.len();

        assert_eq!((result.is_ok(), inserted_version_count), (true, 1));
    }

    #[tokio::test]
    async fn gated_write_accepts_prior_ids_when_earlier_identical_warnings_are_removed() {
        let (use_cases, repository) = make_note_use_cases(source_version());
        let result = use_cases
            .write_with_warning_dispositions(
                write_body_command("close 45,678円"),
                vec![NoteBodyWarningDisposition {
                    id: "price:45%2C678%E5%86%86:2".into(),
                    reason: "分析上の仮定として記載".into(),
                }],
            )
            .await;
        let inserted_version_count = repository.inserted_versions.lock().await.len();

        assert_eq!((result.is_ok(), inserted_version_count), (true, 1));
    }

    #[tokio::test]
    async fn gated_write_rejects_when_only_some_warnings_have_dispositions() {
        let body = "close 45,678円 and -3.5%";
        let warnings = scan_note_body_warnings_with_ids(body);
        let (use_cases, repository) = make_note_use_cases(source_version());
        let result = use_cases
            .write_with_warning_dispositions(
                write_body_command(body),
                vec![NoteBodyWarningDisposition {
                    id: warnings[0].id.clone(),
                    reason: "分析上の仮定として記載".into(),
                }],
            )
            .await;

        assert_rejected_without_saving(result, &repository, warnings.into_iter().skip(1).collect())
            .await;
    }

    #[tokio::test]
    async fn gated_write_rejects_a_disposition_for_a_different_warning() {
        let body = "close 45,678円";
        let mismatched_warning = scan_note_body_warnings_with_ids("close 98,765円")
            .into_iter()
            .next()
            .expect("other body has a warning");
        let (use_cases, repository) = make_note_use_cases(source_version());
        let result = use_cases
            .write_with_warning_dispositions(
                write_body_command(body),
                vec![NoteBodyWarningDisposition {
                    id: mismatched_warning.id,
                    reason: "分析上の仮定として記載".into(),
                }],
            )
            .await;

        assert_rejected_without_saving(result, &repository, scan_note_body_warnings_with_ids(body))
            .await;
    }

    #[rstest]
    #[case::empty_reason("")]
    #[case::whitespace_reason("  ")]
    #[tokio::test]
    async fn gated_write_requires_a_nonempty_reason(#[case] reason: &str) {
        let body = "close 45,678円";
        let warning = scan_note_body_warnings_with_ids(body)
            .into_iter()
            .next()
            .expect("body has a warning");
        let (use_cases, repository) = make_note_use_cases(source_version());
        let result = use_cases
            .write_with_warning_dispositions(
                write_body_command(body),
                vec![NoteBodyWarningDisposition {
                    id: warning.id,
                    reason: reason.into(),
                }],
            )
            .await;
        assert_rejected_without_saving(result, &repository, scan_note_body_warnings_with_ids(body))
            .await;
    }

    async fn assert_rejected_without_saving(
        result: Result<NoteWriteResult, NoteUseCaseError>,
        repository: &LinkNoteRepository,
        expected_warnings: Vec<NoteBodyWarning>,
    ) {
        let unresolved_warnings = result.err().and_then(|error| match error {
            NoteUseCaseError::UnresolvedBodyWarnings { warnings } => Some(warnings),
            _ => None,
        });
        let inserted_version_count = repository.inserted_versions.lock().await.len();

        assert_eq!(
            (unresolved_warnings, inserted_version_count),
            (Some(expected_warnings), 0),
        );
    }

    #[tokio::test]
    async fn ungated_write_keeps_existing_warning_behavior() {
        let (use_cases, repository) = make_note_use_cases(source_version());
        let result = use_cases.write(write_body_command("close 45,678円")).await;
        let inserted_version_count = repository.inserted_versions.lock().await.len();

        assert_eq!((result.is_ok(), inserted_version_count), (true, 1));
    }

    #[tokio::test]
    async fn write_rejects_change_when_start_value_is_zero() {
        let bars_repository = Arc::new(FakeBarsRepository::new());
        bars_repository
            .seed_bars(vec![
                daily_bar("fictional-code", "2030-01-02T00:00:00Z", 0, 11, 0, 0, 100),
                fictional_code_daily_bars()[1].clone(),
            ])
            .await;
        let (use_cases, repository) =
            make_note_use_cases_with_bars(source_version(), bars_repository);
        let result = use_cases
            .write(write_body_command(
                "[[change:fictional-code@2030-01-02..2030-01-03:close]]",
            ))
            .await;
        let inserted_version_count = repository.inserted_versions.lock().await.len();

        assert_eq!(
            (
                result.err().map(|error| error.to_string()),
                inserted_version_count,
            ),
            (
                Some(
                    "価格参照 [[change:fictional-code@2030-01-02..2030-01-03:close]] の開始値は 0 です"
                        .into(),
                ),
                0,
            ),
        );
    }

    #[tokio::test]
    async fn write_rejects_missing_graph_references_for_approval_required_notes() {
        let (use_cases, repository) = make_approval_note_use_cases(source_version());
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
        let replacements = repository.reference_replacements.lock().await.clone();

        assert_eq!(
            (
                result.err().map(|error| error.to_string()),
                inserted_versions,
                replacements,
            ),
            (
                Some(indoc! {"
                    ノートのトークンに問題があります:
                    - 本文のトークン \"[[graph:missing-chart]]\": 対応する graphs[].id がありません
                    許可される形式: `[[stock:<id>]]`, `[[indicator:<id>]]`, `[[group:<axis-key>/<group-key>]]`, `[[note:<uuid>]]`, `[[note:<uuid>@current]]`, `[[anno:<id>]]`, `[[price:<id>@<date>:<field>]]`, `[[change:<id>@<start>..<end>:<field>]]`, `[[valuation:<id>@<date>:<field>]]`, `[[indicator:<id>@<date>]]`, `[[eval:<evaluation-uuid>:<output-name>]]`。`[[graph:<id>]]` は graphs[].id に存在し、空行区切りブロック内で単独にしてください。graphs[].nodes[].ref では参照 3 種のみ使用できます。
                "}
                .trim_end()
                .to_string()),
                0,
                vec![],
            ),
        );
    }

    #[tokio::test]
    async fn write_keeps_pending_graph_references_out_of_the_reference_table() {
        let (use_cases, repository) = make_approval_note_use_cases(source_version());
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
                vec![],
            ),
        );
    }

    #[tokio::test]
    async fn write_rejects_graphs_only_updates_that_remove_a_referenced_graph() {
        let mut source_version = source_version();
        source_version.body_md = "[[graph:sample-chart]]".into();
        source_version.graphs_json = json!([{
            "id": "sample-chart",
            "layout": "flow",
            "nodes": [],
            "edges": [],
        }]);
        let (use_cases, repository) = make_approval_note_use_cases(source_version);
        let result = use_cases
            .write(NoteWriteCommand {
                execution_id: None,
                note_id: Some(SOURCE_NOTE_ID),
                title: Some("Updated title".into()),
                body_md: None,
                frontmatter_json: None,
                graphs_json: Some(json!([])),
                kind: None,
                status: None,
                trigger: None,
                trigger_label: None,
                created_by_kind: "llm".into(),
                change_reason: Some("Update graph definition".into()),
                actor: Actor::Llm { label: "analyst" },
                change_diff: None,
            })
            .await
            .map(|_| ())
            .map_err(|error| matches!(error, NoteUseCaseError::Validation(_)));
        let inserted_versions = repository.inserted_versions.lock().await.len();
        let replacements = repository.reference_replacements.lock().await.clone();

        assert_eq!(
            (result, inserted_versions, replacements),
            (Err(true), 0, vec![])
        );
    }
}
