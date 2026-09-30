use serde_json::json;
use uuid::Uuid;

use crate::change_history::{ChangeHistoryRecord, Op, TargetKind};
use crate::unit_of_work::{SharedUnitOfWork, UnitOfWorkTransaction};

use super::error::CommentUseCaseError;
use super::repository::SharedCommentRepository;
use super::types::{
    Comment, CommentTargetKind, CreateCommentCommand, DeleteCommentCommand, NewComment,
    ReplyCommentCommand, ResolveCommentCommand,
};

const ALLOWED_AUTHOR_KINDS: [&str; 2] = ["human", "llm"];

#[derive(Clone)]
pub struct CommentUseCases {
    unit_of_work: SharedUnitOfWork,
    repository: SharedCommentRepository,
    change_history: crate::change_history::SharedChangeHistoryPort,
}

impl CommentUseCases {
    pub fn new(
        unit_of_work: SharedUnitOfWork,
        repository: SharedCommentRepository,
        change_history: crate::change_history::SharedChangeHistoryPort,
    ) -> Self {
        Self {
            unit_of_work,
            repository,
            change_history,
        }
    }

    pub async fn create(
        &self,
        command: CreateCommentCommand,
    ) -> Result<Comment, CommentUseCaseError> {
        validate_body(&command.body)?;
        validate_author_kind(&command.author_kind)?;
        let target_kind = parse_target_kind(&command.target_kind)?;

        let transaction = self.unit_of_work.begin().await?;
        if let Some(parent_id) = command.parent_id {
            let parent = self
                .repository
                .find_by_id(&transaction, parent_id)
                .await?
                .ok_or_else(|| {
                    CommentUseCaseError::Validation(format!("parent comment {parent_id} not found"))
                })?;
            if parent.parent_id.is_some() {
                return Err(CommentUseCaseError::Validation(
                    "cannot reply to a reply; parent_id must reference a top-level comment".into(),
                ));
            }
            if parent.target_kind != command.target_kind || parent.target_id != command.target_id {
                return Err(CommentUseCaseError::Validation(
                    "parent comment belongs to a different target".into(),
                ));
            }
        }
        self.ensure_scope(&transaction, command.scope, target_kind, command.target_id)
            .await?;
        let (start_line, end_line) = self
            .validate_anchor(
                &transaction,
                target_kind,
                command.target_id,
                command.anchor_side.as_deref(),
                command.start_line,
                command.end_line,
            )
            .await?;
        let anchor_text = normalize_anchor_text(target_kind, command.anchor_text);
        let id = Uuid::new_v4();
        let created = self
            .repository
            .insert(
                &transaction,
                NewComment {
                    id,
                    target_kind,
                    target_id: command.target_id,
                    parent_id: command.parent_id,
                    body: command.body,
                    author_kind: command.author_kind,
                    author_label: command.author_label,
                    anchor_text,
                    anchor_side: command.anchor_side,
                    start_line,
                    end_line,
                },
            )
            .await?;
        self.record_create(&transaction, &created, command.actor)
            .await?;
        self.unit_of_work.commit(transaction).await?;
        Ok(created)
    }

    pub async fn reply(
        &self,
        command: ReplyCommentCommand,
    ) -> Result<Comment, CommentUseCaseError> {
        validate_body(&command.body)?;
        validate_author_kind(&command.author_kind)?;

        let transaction = self.unit_of_work.begin().await?;
        let parent = self
            .repository
            .find_by_id(&transaction, command.parent_id)
            .await?
            .ok_or_else(|| {
                CommentUseCaseError::NotFound(format!(
                    "parent comment {} not found",
                    command.parent_id
                ))
            })?;
        if parent.parent_id.is_some() {
            return Err(CommentUseCaseError::Validation(
                "cannot reply to a reply; parent_id must reference a top-level comment".into(),
            ));
        }
        let target_kind = parse_target_kind(&parent.target_kind)?;
        self.ensure_scope(&transaction, command.scope, target_kind, parent.target_id)
            .await?;

        let id = Uuid::new_v4();
        let created = self
            .repository
            .insert(
                &transaction,
                NewComment {
                    id,
                    target_kind,
                    target_id: parent.target_id,
                    parent_id: Some(parent.id),
                    body: command.body,
                    author_kind: command.author_kind,
                    author_label: command.author_label,
                    anchor_text: None,
                    anchor_side: None,
                    start_line: None,
                    end_line: None,
                },
            )
            .await?;
        self.record_create(&transaction, &created, command.actor)
            .await?;
        self.unit_of_work.commit(transaction).await?;
        Ok(created)
    }

    pub async fn resolve(
        &self,
        command: ResolveCommentCommand,
    ) -> Result<Comment, CommentUseCaseError> {
        let transaction = self.unit_of_work.begin().await?;
        let current = self
            .repository
            .find_by_id(&transaction, command.id)
            .await?
            .ok_or_else(|| {
                CommentUseCaseError::NotFound(format!("comment {} not found", command.id))
            })?;
        let target_kind = parse_target_kind(&current.target_kind)?;
        self.ensure_scope(&transaction, command.scope, target_kind, current.target_id)
            .await?;
        if current.resolved == command.resolved {
            self.unit_of_work.commit(transaction).await?;
            return Ok(current);
        }

        let updated = self
            .repository
            .update_resolved(&transaction, command.id, command.resolved)
            .await?
            .ok_or_else(|| {
                CommentUseCaseError::NotFound(format!("comment {} not found", command.id))
            })?;
        self.change_history
            .record(
                &transaction,
                ChangeHistoryRecord {
                    actor: command.actor,
                    target_kind: TargetKind::Comment,
                    target_id: command.id,
                    op: Op::StatusChange,
                    diff: json!({ "from": current.resolved, "to": command.resolved }),
                    summary: None,
                },
            )
            .await?;
        self.unit_of_work.commit(transaction).await?;
        Ok(updated)
    }

    pub async fn delete(&self, command: DeleteCommentCommand) -> Result<(), CommentUseCaseError> {
        let transaction = self.unit_of_work.begin().await?;
        let current = self
            .repository
            .find_by_id(&transaction, command.id)
            .await?
            .ok_or_else(|| {
                CommentUseCaseError::NotFound(format!("comment {} not found", command.id))
            })?;
        let target_kind = parse_target_kind(&current.target_kind)?;
        self.ensure_scope(&transaction, command.scope, target_kind, current.target_id)
            .await?;
        if !self.repository.delete(&transaction, command.id).await? {
            return Err(CommentUseCaseError::NotFound(format!(
                "comment {} not found",
                command.id
            )));
        }
        self.change_history
            .record(
                &transaction,
                ChangeHistoryRecord {
                    actor: command.actor,
                    target_kind: TargetKind::Comment,
                    target_id: command.id,
                    op: Op::Delete,
                    diff: json!({}),
                    summary: None,
                },
            )
            .await?;
        self.unit_of_work.commit(transaction).await?;
        Ok(())
    }

    async fn ensure_scope(
        &self,
        transaction: &UnitOfWorkTransaction,
        scope: Option<crate::strategy_scope::StrategyScope>,
        target_kind: CommentTargetKind,
        target_id: Uuid,
    ) -> Result<(), CommentUseCaseError> {
        let Some(scope) = scope else {
            return Ok(());
        };
        let target_strategy_id = self
            .repository
            .target_strategy_id(transaction, target_kind, target_id)
            .await?
            .ok_or_else(|| CommentUseCaseError::NotFound("comment target not found".into()))?;
        if target_strategy_id != Some(scope.id()) {
            return Err(CommentUseCaseError::Forbidden(
                "comment target belongs to a different strategy".into(),
            ));
        }
        Ok(())
    }

    async fn validate_anchor(
        &self,
        transaction: &UnitOfWorkTransaction,
        target_kind: CommentTargetKind,
        target_id: Uuid,
        anchor_side: Option<&str>,
        start_line: Option<i32>,
        end_line: Option<i32>,
    ) -> Result<(Option<i32>, Option<i32>), CommentUseCaseError> {
        if target_kind != CommentTargetKind::NoteVersion {
            if anchor_side.is_some() || start_line.is_some() || end_line.is_some() {
                return Err(CommentUseCaseError::Validation(
                    "line anchors are only supported for note_version comments".into(),
                ));
            }
            return Ok((None, None));
        }

        let bodies = self
            .repository
            .note_version_anchor_bodies(transaction, target_id)
            .await?
            .ok_or_else(|| {
                CommentUseCaseError::NotFound(format!("note version {target_id} not found"))
            })?;
        let has_line_data = start_line.is_some() || end_line.is_some();
        if anchor_side.is_none() && !has_line_data {
            return Ok((None, None));
        }

        let (Some(anchor_side), Some(start_line), Some(end_line)) =
            (anchor_side, start_line, end_line)
        else {
            return Err(CommentUseCaseError::Validation(
                "anchor_side, start_line, and end_line must be provided together".into(),
            ));
        };
        if !matches!(anchor_side, "old" | "new") {
            return Err(CommentUseCaseError::Validation(
                "anchor_side must be either old or new".into(),
            ));
        }
        if start_line < 1 || end_line < start_line {
            return Err(CommentUseCaseError::Validation(
                "line anchor must be a valid 1-indexed range".into(),
            ));
        }

        let body = if anchor_side == "new" {
            &bodies.current_body
        } else {
            if bodies.version_no <= 1 {
                return Err(CommentUseCaseError::Validation(
                    "the first version has no old-side lines".into(),
                ));
            }
            bodies.previous_body.as_deref().ok_or_else(|| {
                CommentUseCaseError::NotFound(format!(
                    "previous version of note version {target_id} not found"
                ))
            })?
        };
        let line_count = i32::try_from(body.split('\n').count()).map_err(|_| {
            CommentUseCaseError::Validation("selected version has too many lines".into())
        })?;
        if end_line > line_count {
            return Err(CommentUseCaseError::Validation(format!(
                "line anchor ends at {end_line}, but the selected version has {line_count} lines"
            )));
        }
        Ok((Some(start_line), Some(end_line)))
    }

    async fn record_create(
        &self,
        transaction: &UnitOfWorkTransaction,
        created: &Comment,
        actor: crate::change_history::Actor,
    ) -> Result<(), CommentUseCaseError> {
        self.change_history
            .record(
                transaction,
                ChangeHistoryRecord {
                    actor,
                    target_kind: TargetKind::Comment,
                    target_id: created.id,
                    op: Op::Create,
                    diff: json!({
                        "target_kind": created.target_kind,
                        "target_id": created.target_id,
                        "parent_id": created.parent_id,
                    }),
                    summary: None,
                },
            )
            .await?;
        Ok(())
    }
}

fn validate_body(body: &str) -> Result<(), CommentUseCaseError> {
    if body.trim().is_empty() {
        return Err(CommentUseCaseError::Validation(
            "body must not be empty".into(),
        ));
    }
    Ok(())
}

fn validate_author_kind(author_kind: &str) -> Result<(), CommentUseCaseError> {
    if !ALLOWED_AUTHOR_KINDS.contains(&author_kind) {
        return Err(CommentUseCaseError::Validation(format!(
            "invalid author_kind: {author_kind}"
        )));
    }
    Ok(())
}

fn parse_target_kind(value: &str) -> Result<CommentTargetKind, CommentUseCaseError> {
    CommentTargetKind::parse(value)
        .ok_or_else(|| CommentUseCaseError::Validation(format!("invalid target_kind: {value}")))
}

fn normalize_anchor_text(
    target_kind: CommentTargetKind,
    anchor_text: Option<String>,
) -> Option<String> {
    if target_kind == CommentTargetKind::NoteVersion {
        return anchor_text;
    }
    anchor_text
        .map(|text| text.trim().to_string())
        .filter(|text| !text.is_empty())
}
