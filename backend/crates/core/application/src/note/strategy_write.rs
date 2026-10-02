use chrono::Utc;
use serde_json::json;
use uuid::Uuid;

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
        if let Some(scope) = command.scope
            && command.strategy_id != Some(scope.id())
        {
            return Err(NoteUseCaseError::Validation(
                "strategy_id must match the strategy scope".into(),
            ));
        }
        if command.execution_id.is_some()
            && (command.scope.is_none() || command.strategy_id.is_none())
        {
            return Err(NoteUseCaseError::Validation(
                "execution_id requires a strategy scope".into(),
            ));
        }
        if let Some(frontmatter_json) = command.frontmatter_json.as_ref() {
            ensure_frontmatter_object(frontmatter_json)?;
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
            self.ensure_scope(&note, command.scope)?;
            let result = self
                .update_strategy_note(&transaction, note, command)
                .await?;
            self.unit_of_work.commit(transaction).await?;
            return Ok(result);
        }

        let transaction = self.unit_of_work.begin().await?;
        if let Some(strategy_id) = command.strategy_id {
            self.ensure_strategy_exists(&transaction, strategy_id)
                .await?;
        }
        if let Some(execution_id) = command.execution_id.as_deref()
            && let Some(note) = self
                .repository
                .find_note_by_execution_id(
                    &transaction,
                    command.strategy_id.ok_or_else(|| {
                        NoteUseCaseError::Validation(
                            "execution_id requires a strategy scope".into(),
                        )
                    })?,
                    execution_id,
                )
                .await?
        {
            self.ensure_scope(&note, command.scope)?;
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
                    strategy_id: command.strategy_id,
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
                .find_note_by_execution_id(
                    &transaction,
                    command.strategy_id.ok_or_else(|| {
                        NoteUseCaseError::Validation(
                            "execution_id requires a strategy scope".into(),
                        )
                    })?,
                    execution_id,
                )
                .await?
                .ok_or_else(|| {
                    NoteUseCaseError::Conflict(
                        "note disappeared after execution_id conflict".into(),
                    )
                })?;
            self.ensure_scope(&note, command.scope)?;
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
