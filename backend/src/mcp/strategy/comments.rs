//! コメント操作の inner method 実装。
//!
//! 戦略境界の検証はユースケースが担う。

use core_application::change_history::Actor;
use core_application::comment::{
    CommentListQuery, CommentReadQueryError, CommentReadUseCaseError, CommentTargetKind,
    CommentUseCaseError, ReplyCommentCommand, ResolveCommentCommand,
};
use core_application::strategy_scope::StrategyScope;
use rmcp::ErrorData as McpError;

use super::dto::{
    CommentDto, ReadCommentsParams, ReadCommentsResult, ReplyCommentParams, ReplyCommentResult,
    ResolveCommentParams, ResolveCommentResult,
};
use super::{STRATEGY_AGENT_ACTOR, StrategyServer, internal_error, invalid_params};

fn comment_use_case_to_dto(m: core_application::comment::Comment) -> CommentDto {
    CommentDto {
        comment_id: m.id,
        target_kind: m.target_kind,
        target_id: m.target_id,
        parent_id: m.parent_id,
        body: m.body,
        author_kind: m.author_kind,
        author_label: m.author_label,
        resolved: m.resolved,
        created_at: m.created_at,
        anchor_text: m.anchor_text,
        anchor_side: m.anchor_side,
        start_line: m.start_line,
        end_line: m.end_line,
    }
}

impl StrategyServer {
    pub(crate) async fn read_comments_inner(
        &self,
        scope: impl Into<StrategyScope>,
        params: ReadCommentsParams,
    ) -> Result<ReadCommentsResult, McpError> {
        let scope = scope.into();
        let target_kind = CommentTargetKind::parse(&params.target_kind).ok_or_else(|| {
            let expected = CommentTargetKind::ALL.map(CommentTargetKind::as_str);
            invalid_params(format!(
                "invalid target_kind: {} (expected one of {expected:?})",
                params.target_kind
            ))
        })?;
        let comments = self
            .dependencies
            .comment_reads
            .list_comments(
                CommentListQuery {
                    target_kind,
                    target_id: params.target_id,
                    resolved: params.resolved,
                },
                Some(scope),
            )
            .await
            .map_err(comment_read_error)?;
        Ok(ReadCommentsResult {
            comments: comments.into_iter().map(comment_use_case_to_dto).collect(),
        })
    }

    pub(crate) async fn resolve_comment_inner(
        &self,
        scope: impl Into<StrategyScope>,
        params: ResolveCommentParams,
    ) -> Result<ResolveCommentResult, McpError> {
        let updated = self
            .dependencies
            .comments
            .resolve(ResolveCommentCommand {
                scope: Some(scope.into()),
                actor: Actor::Llm { label: "analyst" },
                id: params.comment_id,
                resolved: params.resolved,
            })
            .await
            .map_err(comment_use_case_error)?;
        Ok(ResolveCommentResult {
            comment: comment_use_case_to_dto(updated),
        })
    }

    pub(crate) async fn reply_comment_inner(
        &self,
        scope: impl Into<StrategyScope>,
        params: ReplyCommentParams,
    ) -> Result<ReplyCommentResult, McpError> {
        let created = self
            .dependencies
            .comments
            .reply(ReplyCommentCommand {
                scope: Some(scope.into()),
                actor: Actor::Llm { label: "analyst" },
                parent_id: params.parent_id,
                body: params.body,
                author_kind: STRATEGY_AGENT_ACTOR.into(),
                author_label: "analyst".into(),
            })
            .await
            .map_err(comment_use_case_error)?;
        Ok(ReplyCommentResult {
            comment: comment_use_case_to_dto(created),
        })
    }
}

fn comment_read_error(error: CommentReadUseCaseError) -> McpError {
    match error {
        CommentReadUseCaseError::Query(CommentReadQueryError::Database(error)) => {
            super::persistence_error_to_mcp(error)
        }
        CommentReadUseCaseError::AnnotationRead(error) => {
            super::annotations::annotation_read_error_to_mcp(error)
        }
        CommentReadUseCaseError::NoteRead(error) => super::notes::note_read_error_to_mcp(error),
    }
}

fn comment_use_case_error(error: CommentUseCaseError) -> McpError {
    match error {
        CommentUseCaseError::Validation(message) => invalid_params(message),
        CommentUseCaseError::NotFound(message) => McpError::resource_not_found(message, None),
        CommentUseCaseError::Forbidden(message) => invalid_params(message),
        other => {
            tracing::error!(error = %other, "strategy mcp comment operation failed");
            internal_error(format!("database error: {other}"))
        }
    }
}
