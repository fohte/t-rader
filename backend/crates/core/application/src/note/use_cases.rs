use serde_json::Value;
use uuid::Uuid;

use crate::change_history::{ChangeHistoryRecord, Op, SharedChangeHistoryPort, TargetKind};
use crate::note::NoteUseCaseError;
use crate::note::repository::SharedNoteRepository;
use crate::note::types::{Note, NoteSnapshot};
use crate::strategy_task_step_evidence::SharedStrategyTaskStepEvidenceRepository;
use crate::unit_of_work::{SharedUnitOfWork, UnitOfWorkTransaction};

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
    pub(super) strategy_task_step_evidence: SharedStrategyTaskStepEvidenceRepository,
}

impl NoteUseCases {
    pub fn new(
        unit_of_work: SharedUnitOfWork,
        repository: SharedNoteRepository,
        change_history: SharedChangeHistoryPort,
        strategy_task_step_evidence: SharedStrategyTaskStepEvidenceRepository,
    ) -> Self {
        Self {
            unit_of_work,
            repository,
            change_history,
            strategy_task_step_evidence,
        }
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
