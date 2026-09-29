use chrono::Utc;
use serde_json::json;
use uuid::Uuid;

use crate::change_history::{Actor, Op};
use crate::note::types::NoteVersionUpdate;
use crate::note::{NoteUseCaseError, NoteUseCases};
use crate::unit_of_work::UnitOfWorkTransaction;

const INITIAL_NOTE_STATUS: &str = "unread";
const APPROVED_NOTE_STATUS: &str = "approved";
const REJECTED_NOTE_STATUS: &str = "rejected";

impl NoteUseCases {
    pub async fn approve_version(
        &self,
        note_id: Uuid,
        version_no: i32,
        label: Option<String>,
    ) -> Result<crate::note::types::NoteVersion, NoteUseCaseError> {
        let transaction = self.unit_of_work.begin().await?;
        let version = self
            .require_version(&transaction, note_id, version_no)
            .await?;
        self.ensure_pending_version(&version)?;
        let now = Utc::now().fixed_offset();
        let current = self
            .repository
            .find_current_version(&transaction, note_id)
            .await?;
        let previous_current_id = current.as_ref().map(|current| current.id);
        let updated = if current
            .as_ref()
            .is_some_and(|current| current.version_no > version.version_no)
        {
            let updated = self
                .repository
                .update_version(
                    &transaction,
                    NoteVersionUpdate {
                        id: version.id,
                        is_current: None,
                        status: Some(APPROVED_NOTE_STATUS.into()),
                        reviewed_at: Some(now),
                    },
                )
                .await?;
            self.repository
                .update_note_timestamp(&transaction, note_id, now)
                .await?;
            updated
        } else {
            self.set_current_version(
                &transaction,
                note_id,
                version,
                Some(APPROVED_NOTE_STATUS),
                Some(now),
                current,
            )
            .await?
        };
        self.record_history(
            &transaction,
            Actor::Human,
            note_id,
            Op::StatusChange,
            json!({
                "from": INITIAL_NOTE_STATUS,
                "to": APPROVED_NOTE_STATUS,
                "version_id": updated.id,
                "previous_current_version_id": previous_current_id,
                "label": label,
            }),
            label,
        )
        .await?;
        self.unit_of_work.commit(transaction).await?;
        Ok(updated)
    }

    pub async fn reject_version(
        &self,
        note_id: Uuid,
        version_no: i32,
        label: Option<String>,
        has_line_comments: bool,
    ) -> Result<crate::note::types::NoteVersion, NoteUseCaseError> {
        if !has_line_comments && label.is_none() {
            return Err(NoteUseCaseError::Validation(
                "a rejection reason is required when no line comments are attached".into(),
            ));
        }
        let transaction = self.unit_of_work.begin().await?;
        let version = self
            .require_version(&transaction, note_id, version_no)
            .await?;
        self.ensure_pending_version(&version)?;
        let now = Utc::now().fixed_offset();
        let updated = self
            .repository
            .update_version(
                &transaction,
                NoteVersionUpdate {
                    id: version.id,
                    is_current: None,
                    status: Some(REJECTED_NOTE_STATUS.into()),
                    reviewed_at: Some(now),
                },
            )
            .await?;
        self.repository
            .update_note_timestamp(&transaction, note_id, now)
            .await?;
        self.record_history(
            &transaction,
            Actor::Human,
            note_id,
            Op::StatusChange,
            json!({
                "from": version.status,
                "to": REJECTED_NOTE_STATUS,
                "version_id": version.id,
                "label": label,
            }),
            label,
        )
        .await?;
        self.unit_of_work.commit(transaction).await?;
        Ok(updated)
    }

    pub async fn make_version_current(
        &self,
        note_id: Uuid,
        version_no: i32,
    ) -> Result<crate::note::types::NoteVersion, NoteUseCaseError> {
        let transaction = self.unit_of_work.begin().await?;
        let version = self
            .require_version(&transaction, note_id, version_no)
            .await?;
        if version.status != APPROVED_NOTE_STATUS {
            return Err(NoteUseCaseError::Conflict(format!(
                "note version {note_id}/{version_no} is not approved"
            )));
        }
        let current = self
            .repository
            .find_current_version(&transaction, note_id)
            .await?;
        if current
            .as_ref()
            .is_some_and(|current| current.id == version.id)
        {
            self.unit_of_work.commit(transaction).await?;
            return Ok(version);
        }

        let previous_current_id = current.as_ref().map(|current| current.id);
        let updated = self
            .set_current_version(&transaction, note_id, version, None, None, current)
            .await?;
        self.record_history(
            &transaction,
            Actor::Human,
            note_id,
            Op::Update,
            json!({
                "from_version_id": previous_current_id,
                "to_version_id": updated.id,
                "version_no": updated.version_no,
            }),
            None,
        )
        .await?;
        self.unit_of_work.commit(transaction).await?;
        Ok(updated)
    }

    async fn require_version(
        &self,
        transaction: &UnitOfWorkTransaction,
        note_id: Uuid,
        version_no: i32,
    ) -> Result<crate::note::types::NoteVersion, NoteUseCaseError> {
        self.repository
            .find_version_by_number(transaction, note_id, version_no)
            .await?
            .ok_or_else(|| {
                NoteUseCaseError::NotFound(format!("note version {note_id}/{version_no} not found"))
            })
    }

    fn ensure_pending_version(
        &self,
        version: &crate::note::types::NoteVersion,
    ) -> Result<(), NoteUseCaseError> {
        if version.status != INITIAL_NOTE_STATUS {
            return Err(NoteUseCaseError::Conflict(format!(
                "note version {}/{} is not pending",
                version.note_id, version.version_no
            )));
        }
        Ok(())
    }

    async fn set_current_version(
        &self,
        transaction: &UnitOfWorkTransaction,
        note_id: Uuid,
        version: crate::note::types::NoteVersion,
        status: Option<&str>,
        reviewed_at: Option<chrono::DateTime<chrono::FixedOffset>>,
        current: Option<crate::note::types::NoteVersion>,
    ) -> Result<crate::note::types::NoteVersion, NoteUseCaseError> {
        if let Some(current) = current.as_ref().filter(|current| current.id != version.id) {
            self.repository
                .update_version(
                    transaction,
                    NoteVersionUpdate {
                        id: current.id,
                        is_current: Some(false),
                        status: None,
                        reviewed_at: None,
                    },
                )
                .await?;
        }
        let updated = self
            .repository
            .update_version(
                transaction,
                NoteVersionUpdate {
                    id: version.id,
                    is_current: Some(true),
                    status: status.map(str::to_string),
                    reviewed_at,
                },
            )
            .await?;
        let note = self.require_note(transaction, note_id).await?;
        let now = Utc::now().fixed_offset();
        self.repository
            .update_note_timestamp(transaction, note_id, now)
            .await?;
        let content_changed = current.as_ref().is_none_or(|current| {
            current.body_md != updated.body_md || current.graphs_json != updated.graphs_json
        });
        if content_changed {
            self.sync_note_references(
                transaction,
                note.id,
                &updated.body_md,
                &updated.graphs_json,
                core_domain::note_reference::BodyTokenPolicy::Validate,
            )
            .await?;
        }
        Ok(updated)
    }
}
