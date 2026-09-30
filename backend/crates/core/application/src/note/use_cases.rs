use serde_json::Value;
use std::collections::HashMap;
use uuid::Uuid;

use crate::change_history::{ChangeHistoryRecord, Op, SharedChangeHistoryPort, TargetKind};
use crate::note::NoteRepositoryError;
use crate::note::NoteUseCaseError;
use crate::note::repository::SharedNoteRepository;
use crate::note::types::{Note, NoteSnapshot, NoteVersion};
use crate::strategy_existence::SharedStrategyExistence;
use crate::strategy_scope::StrategyScope;
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
    pub(super) strategy_existence: SharedStrategyExistence,
    pub(super) change_history: SharedChangeHistoryPort,
}

impl NoteUseCases {
    pub fn new(
        unit_of_work: SharedUnitOfWork,
        repository: SharedNoteRepository,
        strategy_existence: SharedStrategyExistence,
        change_history: SharedChangeHistoryPort,
    ) -> Self {
        Self {
            unit_of_work,
            repository,
            strategy_existence,
            change_history,
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

    pub(crate) async fn find_note_in_transaction(
        &self,
        transaction: &UnitOfWorkTransaction,
        note_id: Uuid,
    ) -> Result<Option<Note>, NoteRepositoryError> {
        self.repository.find_note(transaction, note_id).await
    }

    pub(crate) async fn find_notes_by_ids_in_transaction(
        &self,
        transaction: &UnitOfWorkTransaction,
        note_ids: &[Uuid],
    ) -> Result<Vec<Note>, NoteRepositoryError> {
        self.repository
            .find_notes_by_ids(transaction, note_ids)
            .await
    }

    pub(crate) async fn find_versions_by_ids_in_transaction(
        &self,
        transaction: &UnitOfWorkTransaction,
        version_ids: &[Uuid],
    ) -> Result<Vec<NoteVersion>, NoteRepositoryError> {
        self.repository
            .find_versions_by_ids(transaction, version_ids)
            .await
    }

    pub(crate) async fn find_current_version_in_transaction(
        &self,
        transaction: &UnitOfWorkTransaction,
        note_id: Uuid,
    ) -> Result<Option<NoteVersion>, NoteRepositoryError> {
        self.repository
            .find_current_version(transaction, note_id)
            .await
    }

    pub(crate) async fn find_initial_created_by_kind_by_note_ids_in_transaction(
        &self,
        transaction: &UnitOfWorkTransaction,
        note_ids: &[Uuid],
    ) -> Result<HashMap<Uuid, String>, NoteRepositoryError> {
        self.repository
            .find_initial_created_by_kind_by_note_ids(transaction, note_ids)
            .await
    }

    pub(super) fn ensure_scope(
        &self,
        note: &Note,
        scope: Option<StrategyScope>,
    ) -> Result<(), NoteUseCaseError> {
        if let Some(scope) = scope
            && note.strategy_id != Some(scope.id())
        {
            return Err(NoteUseCaseError::Forbidden(note.id));
        }
        Ok(())
    }

    pub(super) async fn ensure_strategy_exists(
        &self,
        transaction: &UnitOfWorkTransaction,
        strategy_id: Uuid,
    ) -> Result<(), NoteUseCaseError> {
        if !self
            .strategy_existence
            .exists(transaction, strategy_id)
            .await?
        {
            return Err(NoteUseCaseError::Validation(format!(
                "strategy {strategy_id} does not exist"
            )));
        }
        Ok(())
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
