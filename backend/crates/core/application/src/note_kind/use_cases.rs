use serde_json::{Map, json};
use uuid::Uuid;

use crate::change_history::{Actor, ChangeHistoryRecord, Op, SharedChangeHistoryPort, TargetKind};
use crate::note::NoteUseCases;
use crate::persistence::PersistenceError;
use crate::unit_of_work::SharedUnitOfWork;

use super::error::NoteKindUseCaseError;
use super::repository::{NoteKindRepositoryError, SharedNoteKindRepository};
use super::types::{CreateNoteKindCommand, NewNoteKind, NoteKind, UpdateNoteKindCommand};

#[derive(Clone)]
pub struct NoteKindUseCases {
    unit_of_work: SharedUnitOfWork,
    repository: SharedNoteKindRepository,
    change_history: SharedChangeHistoryPort,
    notes: NoteUseCases,
}

impl NoteKindUseCases {
    pub fn new(
        unit_of_work: SharedUnitOfWork,
        repository: SharedNoteKindRepository,
        change_history: SharedChangeHistoryPort,
        notes: NoteUseCases,
    ) -> Self {
        Self {
            unit_of_work,
            repository,
            change_history,
            notes,
        }
    }

    pub async fn list(&self) -> Result<Vec<NoteKind>, NoteKindUseCaseError> {
        self.repository.list().await.map_err(Into::into)
    }

    pub async fn create(
        &self,
        actor: Actor,
        command: CreateNoteKindCommand,
    ) -> Result<NoteKind, NoteKindUseCaseError> {
        let key = validate_key(&command.key)?;
        let display_name = validate_display_name(&command.display_name)?;
        let transaction = self.unit_of_work.begin().await?;
        let created = match self
            .repository
            .insert(
                &transaction,
                NewNoteKind {
                    key: key.clone(),
                    display_name,
                    requires_approval: command.requires_approval,
                    description: command.description,
                    sort_order: command.sort_order.unwrap_or_default(),
                },
            )
            .await
        {
            Ok(created) => created,
            Err(NoteKindRepositoryError::Database(PersistenceError::Conflict(_))) => {
                return Err(NoteKindUseCaseError::Conflict(format!(
                    "note kind {key} already exists"
                )));
            }
            Err(error) => return Err(error.into()),
        };

        self.change_history
            .record(
                &transaction,
                ChangeHistoryRecord {
                    actor,
                    target_kind: TargetKind::NoteKind,
                    target_id: history_target_id(&key),
                    op: Op::Create,
                    diff: json!({
                        "key": created.key,
                        "display_name": created.display_name,
                        "requires_approval": created.requires_approval,
                        "description": created.description,
                        "sort_order": created.sort_order,
                    }),
                    summary: Some(format!("created note kind {key}")),
                },
            )
            .await?;
        self.unit_of_work.commit(transaction).await?;
        Ok(created)
    }

    pub async fn update(
        &self,
        actor: Actor,
        key: &str,
        command: UpdateNoteKindCommand,
    ) -> Result<NoteKind, NoteKindUseCaseError> {
        let key = validate_key(key)?;
        let transaction = self.unit_of_work.begin().await?;
        let current = self
            .repository
            .find_by_key(&transaction, &key)
            .await?
            .ok_or_else(|| NoteKindUseCaseError::NotFound(format!("note kind {key} not found")))?;
        let mut next = current.clone();
        let mut diff = Map::new();

        if let Some(display_name) = command.display_name {
            let display_name = validate_display_name(&display_name)?;
            diff.insert(
                "display_name".into(),
                json!({ "from": current.display_name, "to": display_name }),
            );
            next.display_name = display_name;
        }
        if let Some(requires_approval) = command.requires_approval {
            diff.insert(
                "requires_approval".into(),
                json!({ "from": current.requires_approval, "to": requires_approval }),
            );
            next.requires_approval = requires_approval;
        }
        if let Some(description) = command.description {
            diff.insert(
                "description".into(),
                json!({ "from": current.description, "to": description }),
            );
            next.description = description;
        }
        if let Some(sort_order) = command.sort_order {
            diff.insert(
                "sort_order".into(),
                json!({ "from": current.sort_order, "to": sort_order }),
            );
            next.sort_order = sort_order;
        }

        if diff.is_empty() {
            self.unit_of_work.commit(transaction).await?;
            return Ok(current);
        }

        let updated = self.repository.update(&transaction, next).await?;
        if current.requires_approval && !updated.requires_approval {
            self.notes
                .approve_pending_versions_for_kind(&transaction, &key, actor)
                .await?;
        }
        self.change_history
            .record(
                &transaction,
                ChangeHistoryRecord {
                    actor,
                    target_kind: TargetKind::NoteKind,
                    target_id: history_target_id(&key),
                    op: Op::Update,
                    diff: serde_json::Value::Object(diff),
                    summary: Some(format!("updated note kind {key}")),
                },
            )
            .await?;
        self.unit_of_work.commit(transaction).await?;
        Ok(updated)
    }

    pub async fn delete(&self, actor: Actor, key: &str) -> Result<(), NoteKindUseCaseError> {
        let key = validate_key(key)?;
        let transaction = self.unit_of_work.begin().await?;
        self.repository
            .find_by_key(&transaction, &key)
            .await?
            .ok_or_else(|| NoteKindUseCaseError::NotFound(format!("note kind {key} not found")))?;

        if self.repository.is_used_by_notes(&transaction, &key).await? {
            return Err(NoteKindUseCaseError::Conflict(format!(
                "note kind {key} is used by existing notes"
            )));
        }

        if !self.repository.delete(&transaction, &key).await? {
            return Err(NoteKindUseCaseError::NotFound(format!(
                "note kind {key} not found"
            )));
        }
        self.change_history
            .record(
                &transaction,
                ChangeHistoryRecord {
                    actor,
                    target_kind: TargetKind::NoteKind,
                    target_id: history_target_id(&key),
                    op: Op::Delete,
                    diff: json!({ "key": key }),
                    summary: Some(format!("deleted note kind {key}")),
                },
            )
            .await?;
        self.unit_of_work.commit(transaction).await?;
        Ok(())
    }
}

fn validate_key(key: &str) -> Result<String, NoteKindUseCaseError> {
    if key.trim().is_empty() {
        return Err(NoteKindUseCaseError::Validation(
            "key must not be empty".into(),
        ));
    }
    Ok(key.to_string())
}

fn validate_display_name(display_name: &str) -> Result<String, NoteKindUseCaseError> {
    let display_name = display_name.trim();
    if display_name.is_empty() {
        return Err(NoteKindUseCaseError::Validation(
            "display_name must not be empty".into(),
        ));
    }
    Ok(display_name.to_string())
}

fn history_target_id(key: &str) -> Uuid {
    Uuid::new_v5(&Uuid::NAMESPACE_OID, format!("note_kind:{key}").as_bytes())
}
