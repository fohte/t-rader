use serde_json::{Map, json};
use uuid::Uuid;

use crate::change_history::{Actor, ChangeHistoryRecord, Op, SharedChangeHistoryPort, TargetKind};
use crate::persistence::PersistenceError;
use crate::unit_of_work::SharedUnitOfWork;

use super::error::NoteTagUseCaseError;
use super::repository::{NoteTagRepositoryError, SharedNoteTagRepository};
use super::types::{CreateNoteTagCommand, NewNoteTag, NoteTag, UpdateNoteTagCommand};

#[derive(Clone)]
pub struct NoteTagUseCases {
    unit_of_work: SharedUnitOfWork,
    repository: SharedNoteTagRepository,
    change_history: SharedChangeHistoryPort,
}

impl NoteTagUseCases {
    pub fn new(
        unit_of_work: SharedUnitOfWork,
        repository: SharedNoteTagRepository,
        change_history: SharedChangeHistoryPort,
    ) -> Self {
        Self {
            unit_of_work,
            repository,
            change_history,
        }
    }

    pub async fn list(&self) -> Result<Vec<NoteTag>, NoteTagUseCaseError> {
        self.repository.list().await.map_err(Into::into)
    }

    pub async fn create(
        &self,
        actor: Actor,
        command: CreateNoteTagCommand,
    ) -> Result<NoteTag, NoteTagUseCaseError> {
        let key = validate_key(&command.key)?;
        let display_name = validate_display_name(&command.display_name)?;
        let transaction = self.unit_of_work.begin().await?;
        let created = match self
            .repository
            .insert(
                &transaction,
                NewNoteTag {
                    key: key.clone(),
                    display_name,
                    description: command.description,
                },
            )
            .await
        {
            Ok(created) => created,
            Err(NoteTagRepositoryError::Database(PersistenceError::Conflict(_))) => {
                return Err(NoteTagUseCaseError::Conflict(format!(
                    "note tag {key} already exists"
                )));
            }
            Err(error) => return Err(error.into()),
        };

        self.change_history
            .record(
                &transaction,
                ChangeHistoryRecord {
                    actor,
                    target_kind: TargetKind::NoteTag,
                    target_id: history_target_id(&key),
                    op: Op::Create,
                    diff: json!({
                        "key": created.key,
                        "display_name": created.display_name,
                        "description": created.description,
                    }),
                    summary: Some(format!("created note tag {key}")),
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
        command: UpdateNoteTagCommand,
    ) -> Result<NoteTag, NoteTagUseCaseError> {
        let key = validate_key(key)?;
        let transaction = self.unit_of_work.begin().await?;
        let current = self
            .repository
            .find_by_key(&transaction, &key)
            .await?
            .ok_or_else(|| NoteTagUseCaseError::NotFound(format!("note tag {key} not found")))?;
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
        if let Some(description) = command.description {
            diff.insert(
                "description".into(),
                json!({ "from": current.description, "to": description }),
            );
            next.description = description;
        }

        if diff.is_empty() {
            self.unit_of_work.commit(transaction).await?;
            return Ok(current);
        }

        let updated = self.repository.update(&transaction, next).await?;
        self.change_history
            .record(
                &transaction,
                ChangeHistoryRecord {
                    actor,
                    target_kind: TargetKind::NoteTag,
                    target_id: history_target_id(&key),
                    op: Op::Update,
                    diff: serde_json::Value::Object(diff),
                    summary: Some(format!("updated note tag {key}")),
                },
            )
            .await?;
        self.unit_of_work.commit(transaction).await?;
        Ok(updated)
    }

    pub async fn delete(&self, actor: Actor, key: &str) -> Result<(), NoteTagUseCaseError> {
        let key = validate_key(key)?;
        let transaction = self.unit_of_work.begin().await?;
        self.repository
            .find_by_key(&transaction, &key)
            .await?
            .ok_or_else(|| NoteTagUseCaseError::NotFound(format!("note tag {key} not found")))?;

        if !self.repository.delete(&transaction, &key).await? {
            return Err(NoteTagUseCaseError::NotFound(format!(
                "note tag {key} not found"
            )));
        }
        self.change_history
            .record(
                &transaction,
                ChangeHistoryRecord {
                    actor,
                    target_kind: TargetKind::NoteTag,
                    target_id: history_target_id(&key),
                    op: Op::Delete,
                    diff: json!({ "key": key }),
                    summary: Some(format!("deleted note tag {key}")),
                },
            )
            .await?;
        self.unit_of_work.commit(transaction).await?;
        Ok(())
    }
}

fn validate_key(key: &str) -> Result<String, NoteTagUseCaseError> {
    if key.trim().is_empty() {
        return Err(NoteTagUseCaseError::Validation(
            "key must not be empty".into(),
        ));
    }
    Ok(key.to_string())
}

fn validate_display_name(display_name: &str) -> Result<String, NoteTagUseCaseError> {
    let display_name = display_name.trim();
    if display_name.is_empty() {
        return Err(NoteTagUseCaseError::Validation(
            "display_name must not be empty".into(),
        ));
    }
    Ok(display_name.to_string())
}

fn history_target_id(key: &str) -> Uuid {
    Uuid::new_v5(&Uuid::NAMESPACE_OID, format!("note_tag:{key}").as_bytes())
}
