use chrono::Utc;
use serde_json::{Map, Value, json};
use uuid::Uuid;

use crate::change_history::{Actor, Op};
use crate::note::types::{NoteMetadataUpdate, NoteSnapshot, UpdateNoteCommand};
use crate::note::use_cases::ensure_frontmatter_object;
use crate::note::version_write::AppendVersionCommand;
use crate::note::{NoteUseCaseError, NoteUseCases};
use crate::unit_of_work::UnitOfWorkTransaction;

impl NoteUseCases {
    pub async fn update(
        &self,
        note_id: Uuid,
        command: UpdateNoteCommand,
    ) -> Result<NoteSnapshot, NoteUseCaseError> {
        let transaction = self.unit_of_work.begin().await?;
        let current_note = self.require_note(&transaction, note_id).await?;
        let current_version = self
            .repository
            .find_current_version(&transaction, note_id)
            .await?
            .ok_or_else(|| {
                NoteUseCaseError::NotFound(format!("current version for note {note_id} not found"))
            })?;
        let mut diff = Map::new();
        let mut title = current_version.title.clone();
        let mut body_md = current_version.body_md.clone();
        let mut frontmatter_json = current_version.frontmatter_json.clone();
        let mut version_changed = false;

        if let Some(value) = command.title {
            let value = value.trim().to_string();
            if value.is_empty() {
                return Err(NoteUseCaseError::Validation(
                    "title must not be empty".into(),
                ));
            }
            diff.insert(
                "title".into(),
                json!({ "from": current_version.title, "to": value }),
            );
            version_changed = true;
            title = value;
        }
        if let Some(value) = command.body_md {
            diff.insert(
                "body_md".into(),
                json!({ "len_from": current_version.body_md.len(), "len_to": value.len() }),
            );
            body_md = value;
            version_changed = true;
        }
        if let Some(value) = command.frontmatter_json {
            ensure_frontmatter_object(&value)?;
            diff.insert(
                "frontmatter_json".into(),
                json!({ "from": current_version.frontmatter_json, "to": value }),
            );
            frontmatter_json = value;
            version_changed = true;
        }
        if let Some(kind) = command.kind.as_ref() {
            if let Some(key) = kind.as_deref() {
                self.ensure_note_kind_exists(&transaction, key).await?;
            }
            diff.insert(
                "kind".into(),
                json!({ "from": current_note.kind, "to": kind }),
            );
        }
        if let Some(trigger) = command.trigger.as_ref() {
            diff.insert(
                "trigger".into(),
                json!({ "from": current_note.trigger, "to": trigger }),
            );
        }
        if let Some(trigger_label) = command.trigger_label.as_ref() {
            diff.insert(
                "trigger_label".into(),
                json!({ "from": current_note.trigger_label, "to": trigger_label }),
            );
        }

        let has_metadata_update =
            command.kind.is_some() || command.trigger.is_some() || command.trigger_label.is_some();
        if has_metadata_update || !version_changed {
            self.repository
                .update_note(
                    &transaction,
                    NoteMetadataUpdate {
                        id: note_id,
                        kind: command.kind,
                        trigger: command.trigger.map(Some),
                        trigger_label: command.trigger_label.map(Some),
                        updated_at: (!version_changed).then(|| Utc::now().fixed_offset()),
                    },
                )
                .await?;
        }

        if version_changed {
            let note = self.require_note(&transaction, note_id).await?;
            self.append_version(
                &transaction,
                &note,
                AppendVersionCommand {
                    title,
                    body_md,
                    frontmatter_json,
                    graphs_json: current_version.graphs_json,
                    created_by_kind: "human".into(),
                    execution_id: None,
                    change_reason: None,
                    change_diff: Some(Value::Object(diff)),
                    actor: Actor::Human,
                },
            )
            .await?;
        } else if !diff.is_empty() {
            self.record_history(
                &transaction,
                Actor::Human,
                note_id,
                Op::Update,
                Value::Object(diff),
                None,
            )
            .await?;
        }

        let snapshot = self.snapshot(&transaction, note_id).await?;
        self.unit_of_work.commit(transaction).await?;
        Ok(snapshot)
    }

    pub async fn delete(&self, note_id: Uuid) -> Result<(), NoteUseCaseError> {
        let transaction = self.unit_of_work.begin().await?;
        if !self.repository.delete_note(&transaction, note_id).await? {
            return Err(NoteUseCaseError::NotFound(format!(
                "note {note_id} not found"
            )));
        }
        self.record_history(
            &transaction,
            Actor::Human,
            note_id,
            Op::Delete,
            json!({}),
            None,
        )
        .await?;
        self.unit_of_work.commit(transaction).await?;
        Ok(())
    }

    pub(super) async fn ensure_note_kind_exists(
        &self,
        transaction: &UnitOfWorkTransaction,
        kind: &str,
    ) -> Result<(), NoteUseCaseError> {
        if self
            .repository
            .find_note_kind_requires_approval(transaction, kind)
            .await?
            .is_none()
        {
            return Err(NoteUseCaseError::UnknownNoteKind(kind.to_string()));
        }
        Ok(())
    }
}
