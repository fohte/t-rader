use serde_json::Value;
use uuid::Uuid;

use crate::bars::BarsUseCases;
use crate::change_history::{ChangeHistoryRecord, Op, SharedChangeHistoryPort, TargetKind};
use crate::custom_indicator_evaluation::CustomIndicatorEvaluationUseCases;
use crate::indicator_observation::IndicatorObservationUseCases;
use crate::markdown_lint::MarkdownLintService;
use crate::note::NoteUseCaseError;
use crate::note::repository::SharedNoteRepository;
use crate::note::types::{Note, NoteSnapshot};
use crate::unit_of_work::{SharedUnitOfWork, UnitOfWorkTransaction};
use crate::valuation::ValuationUseCases;

pub(super) fn ensure_frontmatter_object(value: &Value) -> Result<(), NoteUseCaseError> {
    if value.is_object() {
        Ok(())
    } else {
        Err(NoteUseCaseError::Validation(
            "frontmatter_json must be a JSON object".into(),
        ))
    }
}

#[derive(Clone)]
pub struct NoteUseCases {
    pub(super) unit_of_work: SharedUnitOfWork,
    pub(super) repository: SharedNoteRepository,
    pub(super) change_history: SharedChangeHistoryPort,
    pub(super) bars: BarsUseCases,
    pub(super) valuations: ValuationUseCases,
    pub(super) indicator_observations: IndicatorObservationUseCases,
    pub(super) custom_indicator_evaluations: CustomIndicatorEvaluationUseCases,
    pub(super) markdown_lint: MarkdownLintService,
}

impl NoteUseCases {
    pub fn new(
        unit_of_work: SharedUnitOfWork,
        repository: SharedNoteRepository,
        change_history: SharedChangeHistoryPort,
        bars: BarsUseCases,
        valuations: ValuationUseCases,
        indicator_observations: IndicatorObservationUseCases,
        custom_indicator_evaluations: CustomIndicatorEvaluationUseCases,
    ) -> Self {
        Self {
            unit_of_work,
            repository,
            change_history,
            bars,
            valuations,
            indicator_observations,
            custom_indicator_evaluations,
            markdown_lint: MarkdownLintService::default(),
        }
    }

    pub fn with_markdown_lint_service(mut self, markdown_lint: MarkdownLintService) -> Self {
        self.markdown_lint = markdown_lint;
        self
    }

    pub(super) async fn require_note(
        &self,
        transaction: &UnitOfWorkTransaction,
        note_id: Uuid,
    ) -> Result<Note, NoteUseCaseError> {
        self.repository
            .find_note(transaction, note_id)
            .await?
            .ok_or_else(|| NoteUseCaseError::NotFound(format!("note {note_id} not found")))
    }

    pub(super) async fn snapshot(
        &self,
        transaction: &UnitOfWorkTransaction,
        note_id: Uuid,
    ) -> Result<NoteSnapshot, NoteUseCaseError> {
        let note = self.require_note(transaction, note_id).await?;
        let version = self
            .repository
            .find_current_version(transaction, note_id)
            .await?
            .ok_or_else(|| {
                NoteUseCaseError::NotFound(format!("current version for note {note_id} not found"))
            })?;
        let created_by_kind = self
            .repository
            .find_initial_created_by_kind(transaction, note_id)
            .await?
            .ok_or_else(|| {
                NoteUseCaseError::NotFound(format!("initial version for note {note_id} not found"))
            })?;
        Ok(NoteSnapshot {
            note,
            version,
            created_by_kind,
        })
    }

    pub(super) async fn record_history(
        &self,
        transaction: &UnitOfWorkTransaction,
        actor: crate::change_history::Actor,
        target_id: Uuid,
        op: Op,
        diff: Value,
        summary: Option<String>,
    ) -> Result<(), NoteUseCaseError> {
        self.change_history
            .record(
                transaction,
                ChangeHistoryRecord {
                    actor,
                    target_kind: TargetKind::Note,
                    target_id,
                    op,
                    diff,
                    summary,
                },
            )
            .await?;
        Ok(())
    }
}
