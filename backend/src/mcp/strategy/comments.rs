//! コメント操作の inner method 実装。
//!
//! 書き込み時の戦略境界はユースケースが検証し、読み取り時は対象の所有権をここで検証する。

use core_application::change_history::Actor;
use core_application::comment::{CommentUseCaseError, ReplyCommentCommand, ResolveCommentCommand};
use core_application::note::NoteReadUseCases;
use core_application::strategy_scope::StrategyScope;
use rmcp::ErrorData as McpError;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
use uuid::Uuid;

use gateway_postgres::entities::comment;

use super::dto::{
    CommentDto, ReadCommentsParams, ReadCommentsResult, ReplyCommentParams, ReplyCommentResult,
    ResolveCommentParams, ResolveCommentResult,
};
use super::{
    STRATEGY_AGENT_ACTOR, StrategyServer, db_error, fetch_annotation_owned_by, internal_error,
    invalid_params,
};

const ALLOWED_COMMENT_TARGET_KIND: [&str; 2] = ["note_version", "annotation"];

fn comment_to_dto(m: comment::Model) -> CommentDto {
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

/// comment の target_kind に応じて所有権 (strategy_id 一致) を検査する。
async fn ensure_comment_target_owned_by(
    db: &impl sea_orm::ConnectionTrait,
    note_reads: &NoteReadUseCases,
    target_kind: &str,
    target_id: Uuid,
    scope: StrategyScope,
) -> Result<(), McpError> {
    match target_kind {
        "note_version" => {
            note_reads
                .ensure_note_version_scope(target_id, scope)
                .await
                .map_err(super::notes::note_read_error_to_mcp)?;
        }
        "annotation" => {
            fetch_annotation_owned_by(db, target_id, scope.id()).await?;
        }
        other => {
            return Err(internal_error(format!(
                "comment has unexpected target_kind: {other}"
            )));
        }
    }
    Ok(())
}

impl StrategyServer {
    pub(crate) async fn read_comments_inner(
        &self,
        scope: impl Into<StrategyScope>,
        params: ReadCommentsParams,
    ) -> Result<ReadCommentsResult, McpError> {
        let scope = scope.into();
        if !ALLOWED_COMMENT_TARGET_KIND.contains(&params.target_kind.as_str()) {
            return Err(invalid_params(format!(
                "invalid target_kind: {} (expected one of {ALLOWED_COMMENT_TARGET_KIND:?})",
                params.target_kind
            )));
        }
        ensure_comment_target_owned_by(
            &self.db,
            &self.use_cases.note_reads(),
            &params.target_kind,
            params.target_id,
            scope,
        )
        .await?;

        let mut query = comment::Entity::find()
            .filter(comment::Column::TargetKind.eq(params.target_kind))
            .filter(comment::Column::TargetId.eq(params.target_id));
        if let Some(resolved) = params.resolved {
            query = query.filter(comment::Column::Resolved.eq(resolved));
        }
        let rows = query
            .order_by_asc(comment::Column::CreatedAt)
            .all(&self.db)
            .await
            .map_err(db_error)?;
        Ok(ReadCommentsResult {
            comments: rows.into_iter().map(comment_to_dto).collect(),
        })
    }

    pub(crate) async fn resolve_comment_inner(
        &self,
        scope: impl Into<StrategyScope>,
        params: ResolveCommentParams,
    ) -> Result<ResolveCommentResult, McpError> {
        let updated = self
            .use_cases
            .comments()
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
            .use_cases
            .comments()
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

#[cfg(test)]
mod tests {
    use super::super::dto::{
        CommentDto, ReadCommentsParams, ReplyCommentParams, ResolveCommentParams,
    };
    use super::super::tests_common::{
        ChangeHistoryShape, build_server, change_history_for, current_note_version_id,
        insert_strategy, normalize_comment, seed_comment, seed_foreign_annotation,
        seed_foreign_note, ts_sentinel,
    };
    use gateway_postgres::entities::comment;
    use sea_orm::ActiveModelTrait;
    use sea_orm::ActiveValue::Set;

    #[backend_test_macros::database_test]
    async fn read_comments_returns_target_comments_in_thread_order(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let strategy_id = insert_strategy(&db, "long").await;
        let server = build_server(db.clone());
        let note_id = seed_foreign_note(&db, strategy_id, "note").await;
        let other_note_id = seed_foreign_note(&db, strategy_id, "other note").await;
        let note_version_id = current_note_version_id(&db, note_id).await;
        let other_version_id = current_note_version_id(&db, other_note_id).await;

        let root = seed_comment(&db, "note_version", note_version_id, None, "root comment").await;
        let reply = seed_comment(
            &db,
            "note_version",
            note_version_id,
            Some(root),
            "reply comment",
        )
        .await;
        let comment_time = chrono::DateTime::<chrono::Utc>::UNIX_EPOCH.fixed_offset();
        comment::ActiveModel {
            id: Set(root),
            created_at: Set(comment_time),
            ..Default::default()
        }
        .update(&db)
        .await
        .expect("set root comment time");
        comment::ActiveModel {
            id: Set(reply),
            created_at: Set(comment_time + chrono::Duration::seconds(1)),
            ..Default::default()
        }
        .update(&db)
        .await
        .expect("set reply comment time");
        seed_comment(
            &db,
            "note_version",
            other_version_id,
            None,
            "unrelated comment",
        )
        .await;

        let result = server
            .read_comments_inner(
                strategy_id,
                ReadCommentsParams {
                    target_kind: "note_version".into(),
                    target_id: note_version_id,
                    resolved: None,
                },
            )
            .await
            .expect("read_comments");

        assert_eq!(
            result
                .comments
                .into_iter()
                .map(normalize_comment)
                .collect::<Vec<_>>(),
            vec![
                CommentDto {
                    comment_id: root,
                    target_kind: "note_version".into(),
                    target_id: note_version_id,
                    parent_id: None,
                    body: "root comment".into(),
                    author_kind: "human".into(),
                    author_label: "user".into(),
                    resolved: false,
                    created_at: ts_sentinel(),
                    anchor_text: None,
                    anchor_side: None,
                    start_line: None,
                    end_line: None,
                },
                CommentDto {
                    comment_id: reply,
                    target_kind: "note_version".into(),
                    target_id: note_version_id,
                    parent_id: Some(root),
                    body: "reply comment".into(),
                    author_kind: "human".into(),
                    author_label: "user".into(),
                    resolved: false,
                    created_at: ts_sentinel(),
                    anchor_text: None,
                    anchor_side: None,
                    start_line: None,
                    end_line: None,
                },
            ],
        );
    }

    #[backend_test_macros::database_test]
    async fn read_comments_supports_annotation_target(db: gateway_postgres::DatabaseHandle) {
        let strategy_id = insert_strategy(&db, "swing").await;
        let server = build_server(db.clone());
        let annotation_id = seed_foreign_annotation(&db, strategy_id).await;
        let comment_id = seed_comment(&db, "annotation", annotation_id, None, "looks wrong").await;

        let result = server
            .read_comments_inner(
                strategy_id,
                ReadCommentsParams {
                    target_kind: "annotation".into(),
                    target_id: annotation_id,
                    resolved: None,
                },
            )
            .await
            .expect("read_comments");

        assert_eq!(
            result
                .comments
                .into_iter()
                .map(normalize_comment)
                .collect::<Vec<_>>(),
            vec![CommentDto {
                comment_id,
                target_kind: "annotation".into(),
                target_id: annotation_id,
                parent_id: None,
                body: "looks wrong".into(),
                author_kind: "human".into(),
                author_label: "user".into(),
                resolved: false,
                created_at: ts_sentinel(),
                anchor_text: None,
                anchor_side: None,
                start_line: None,
                end_line: None,
            }],
        );
    }

    #[backend_test_macros::database_test]
    async fn read_comments_rejects_invalid_target_kind(db: gateway_postgres::DatabaseHandle) {
        let strategy_id = insert_strategy(&db, "x").await;
        let server = build_server(db);

        let err = server
            .read_comments_inner(
                strategy_id,
                ReadCommentsParams {
                    target_kind: "garbage".into(),
                    target_id: uuid::Uuid::new_v4(),
                    resolved: None,
                },
            )
            .await
            .expect_err("invalid target_kind expected to be rejected");
        assert_eq!(err.code, rmcp::model::ErrorCode::INVALID_PARAMS);
    }

    #[backend_test_macros::database_test]
    async fn read_comments_rejects_cross_strategy_note(db: gateway_postgres::DatabaseHandle) {
        let strategy_a = insert_strategy(&db, "a").await;
        let strategy_b = insert_strategy(&db, "b").await;
        let server = build_server(db.clone());
        let note_id = seed_foreign_note(&db, strategy_b, "b's note").await;
        let note_version_id = current_note_version_id(&db, note_id).await;

        let err = server
            .read_comments_inner(
                strategy_a,
                ReadCommentsParams {
                    target_kind: "note_version".into(),
                    target_id: note_version_id,
                    resolved: None,
                },
            )
            .await
            .expect_err("cross-strategy note expected to be rejected");
        assert_eq!(err.code, rmcp::model::ErrorCode::INVALID_PARAMS);
    }

    #[backend_test_macros::database_test]
    async fn read_comments_rejects_cross_strategy_annotation(db: gateway_postgres::DatabaseHandle) {
        let strategy_a = insert_strategy(&db, "a").await;
        let strategy_b = insert_strategy(&db, "b").await;
        let server = build_server(db.clone());
        let annotation_id = seed_foreign_annotation(&db, strategy_b).await;

        let err = server
            .read_comments_inner(
                strategy_a,
                ReadCommentsParams {
                    target_kind: "annotation".into(),
                    target_id: annotation_id,
                    resolved: None,
                },
            )
            .await
            .expect_err("cross-strategy annotation expected to be rejected");
        assert_eq!(err.code, rmcp::model::ErrorCode::INVALID_PARAMS);
    }

    #[backend_test_macros::database_test]
    async fn read_comments_filters_by_resolved(db: gateway_postgres::DatabaseHandle) {
        let strategy_id = insert_strategy(&db, "long").await;
        let server = build_server(db.clone());
        let note_id = seed_foreign_note(&db, strategy_id, "note").await;
        let note_version_id = current_note_version_id(&db, note_id).await;
        let open = seed_comment(&db, "note_version", note_version_id, None, "still open").await;
        let done = seed_comment(&db, "note_version", note_version_id, None, "already fixed").await;
        server
            .resolve_comment_inner(
                strategy_id,
                ResolveCommentParams {
                    comment_id: done,
                    resolved: true,
                },
            )
            .await
            .expect("resolve_comment");

        let unresolved_only = server
            .read_comments_inner(
                strategy_id,
                ReadCommentsParams {
                    target_kind: "note_version".into(),
                    target_id: note_version_id,
                    resolved: Some(false),
                },
            )
            .await
            .expect("read_comments");
        assert_eq!(
            unresolved_only
                .comments
                .into_iter()
                .map(normalize_comment)
                .collect::<Vec<_>>(),
            vec![CommentDto {
                comment_id: open,
                target_kind: "note_version".into(),
                target_id: note_version_id,
                parent_id: None,
                body: "still open".into(),
                author_kind: "human".into(),
                author_label: "user".into(),
                resolved: false,
                created_at: ts_sentinel(),
                anchor_text: None,
                anchor_side: None,
                start_line: None,
                end_line: None,
            }],
        );

        let resolved_only = server
            .read_comments_inner(
                strategy_id,
                ReadCommentsParams {
                    target_kind: "note_version".into(),
                    target_id: note_version_id,
                    resolved: Some(true),
                },
            )
            .await
            .expect("read_comments");
        assert_eq!(
            resolved_only
                .comments
                .into_iter()
                .map(normalize_comment)
                .collect::<Vec<_>>(),
            vec![CommentDto {
                comment_id: done,
                target_kind: "note_version".into(),
                target_id: note_version_id,
                parent_id: None,
                body: "already fixed".into(),
                author_kind: "human".into(),
                author_label: "user".into(),
                resolved: true,
                created_at: ts_sentinel(),
                anchor_text: None,
                anchor_side: None,
                start_line: None,
                end_line: None,
            }],
        );
    }

    #[backend_test_macros::database_test]
    async fn resolve_comment_toggles_resolved(db: gateway_postgres::DatabaseHandle) {
        let strategy_id = insert_strategy(&db, "long").await;
        let server = build_server(db.clone());
        let note_id = seed_foreign_note(&db, strategy_id, "note").await;
        let note_version_id = current_note_version_id(&db, note_id).await;
        let comment_id = seed_comment(&db, "note_version", note_version_id, None, "fix this").await;

        let result = server
            .resolve_comment_inner(
                strategy_id,
                ResolveCommentParams {
                    comment_id,
                    resolved: true,
                },
            )
            .await
            .expect("resolve_comment");
        assert_eq!(
            normalize_comment(result.comment),
            CommentDto {
                comment_id,
                target_kind: "note_version".into(),
                target_id: note_version_id,
                parent_id: None,
                body: "fix this".into(),
                author_kind: "human".into(),
                author_label: "user".into(),
                resolved: true,
                created_at: ts_sentinel(),
                anchor_text: None,
                anchor_side: None,
                start_line: None,
                end_line: None,
            },
        );

        server
            .resolve_comment_inner(
                strategy_id,
                ResolveCommentParams {
                    comment_id,
                    resolved: true,
                },
            )
            .await
            .expect("resolving an already resolved comment is a no-op");

        let result = server
            .resolve_comment_inner(
                strategy_id,
                ResolveCommentParams {
                    comment_id,
                    resolved: false,
                },
            )
            .await
            .expect("resolve_comment");
        assert_eq!(
            normalize_comment(result.comment),
            CommentDto {
                comment_id,
                target_kind: "note_version".into(),
                target_id: note_version_id,
                parent_id: None,
                body: "fix this".into(),
                author_kind: "human".into(),
                author_label: "user".into(),
                resolved: false,
                created_at: ts_sentinel(),
                anchor_text: None,
                anchor_side: None,
                start_line: None,
                end_line: None,
            },
        );
        assert_eq!(
            change_history_for(&db, comment_id).await,
            vec![
                ChangeHistoryShape {
                    id: uuid::Uuid::nil(),
                    target_kind: "comment".into(),
                    target_id: comment_id,
                    actor_kind: "llm".into(),
                    actor_label: "analyst".into(),
                    op: "status_change".into(),
                    diff_json: serde_json::json!({ "from": false, "to": true }),
                    summary: None,
                    created_at: ts_sentinel(),
                },
                ChangeHistoryShape {
                    id: uuid::Uuid::nil(),
                    target_kind: "comment".into(),
                    target_id: comment_id,
                    actor_kind: "llm".into(),
                    actor_label: "analyst".into(),
                    op: "status_change".into(),
                    diff_json: serde_json::json!({ "from": true, "to": false }),
                    summary: None,
                    created_at: ts_sentinel(),
                },
            ],
        );
    }

    #[backend_test_macros::database_test]
    async fn resolve_comment_rejects_missing_comment(db: gateway_postgres::DatabaseHandle) {
        let strategy_id = insert_strategy(&db, "x").await;
        let server = build_server(db);

        let err = server
            .resolve_comment_inner(
                strategy_id,
                ResolveCommentParams {
                    comment_id: uuid::Uuid::new_v4(),
                    resolved: true,
                },
            )
            .await
            .expect_err("missing comment expected to be rejected");
        assert_eq!(err.code, rmcp::model::ErrorCode::RESOURCE_NOT_FOUND);
    }

    #[backend_test_macros::database_test]
    async fn resolve_comment_rejects_cross_strategy(db: gateway_postgres::DatabaseHandle) {
        let strategy_a = insert_strategy(&db, "a").await;
        let strategy_b = insert_strategy(&db, "b").await;
        let server = build_server(db.clone());
        let note_id = seed_foreign_note(&db, strategy_b, "b's note").await;
        let note_version_id = current_note_version_id(&db, note_id).await;
        let comment_id = seed_comment(&db, "note_version", note_version_id, None, "fix this").await;

        let err = server
            .resolve_comment_inner(
                strategy_a,
                ResolveCommentParams {
                    comment_id,
                    resolved: true,
                },
            )
            .await
            .expect_err("cross-strategy comment expected to be rejected");
        assert_eq!(err.code, rmcp::model::ErrorCode::INVALID_PARAMS);
    }

    #[backend_test_macros::database_test]
    async fn reply_comment_inherits_parent_target(db: gateway_postgres::DatabaseHandle) {
        let strategy_id = insert_strategy(&db, "long").await;
        let server = build_server(db.clone());
        let note_id = seed_foreign_note(&db, strategy_id, "note").await;
        let note_version_id = current_note_version_id(&db, note_id).await;
        let parent_id =
            seed_comment(&db, "note_version", note_version_id, None, "please fix").await;

        let result = server
            .reply_comment_inner(
                strategy_id,
                ReplyCommentParams {
                    parent_id,
                    body: "fixed in the latest revision".into(),
                },
            )
            .await
            .expect("reply_comment");

        let dto = normalize_comment(result.comment);
        let comment_id = dto.comment_id;
        assert_eq!(
            dto,
            CommentDto {
                comment_id,
                target_kind: "note_version".into(),
                target_id: note_version_id,
                parent_id: Some(parent_id),
                body: "fixed in the latest revision".into(),
                author_kind: super::super::STRATEGY_AGENT_ACTOR.into(),
                author_label: "analyst".into(),
                resolved: false,
                created_at: ts_sentinel(),
                anchor_text: None,
                anchor_side: None,
                start_line: None,
                end_line: None,
            },
        );
        assert_eq!(
            change_history_for(&db, comment_id).await,
            vec![ChangeHistoryShape {
                id: uuid::Uuid::nil(),
                target_kind: "comment".into(),
                target_id: comment_id,
                actor_kind: "llm".into(),
                actor_label: "analyst".into(),
                op: "create".into(),
                diff_json: serde_json::json!({
                    "target_kind": "note_version",
                    "target_id": note_version_id,
                    "parent_id": parent_id,
                }),
                summary: None,
                created_at: ts_sentinel(),
            }],
        );
    }

    #[backend_test_macros::database_test]
    async fn reply_comment_rejects_empty_body(db: gateway_postgres::DatabaseHandle) {
        let strategy_id = insert_strategy(&db, "long").await;
        let server = build_server(db.clone());
        let note_id = seed_foreign_note(&db, strategy_id, "note").await;
        let note_version_id = current_note_version_id(&db, note_id).await;
        let parent_id =
            seed_comment(&db, "note_version", note_version_id, None, "please fix").await;

        let err = server
            .reply_comment_inner(
                strategy_id,
                ReplyCommentParams {
                    parent_id,
                    body: "   ".into(),
                },
            )
            .await
            .expect_err("empty body expected to be rejected");
        assert_eq!(err.code, rmcp::model::ErrorCode::INVALID_PARAMS);
    }

    #[backend_test_macros::database_test]
    async fn reply_comment_rejects_missing_parent(db: gateway_postgres::DatabaseHandle) {
        let strategy_id = insert_strategy(&db, "x").await;
        let server = build_server(db);

        let err = server
            .reply_comment_inner(
                strategy_id,
                ReplyCommentParams {
                    parent_id: uuid::Uuid::new_v4(),
                    body: "fixed".into(),
                },
            )
            .await
            .expect_err("missing parent expected to be rejected");
        assert_eq!(err.code, rmcp::model::ErrorCode::RESOURCE_NOT_FOUND);
    }

    #[backend_test_macros::database_test]
    async fn reply_comment_rejects_cross_strategy(db: gateway_postgres::DatabaseHandle) {
        let strategy_a = insert_strategy(&db, "a").await;
        let strategy_b = insert_strategy(&db, "b").await;
        let server = build_server(db.clone());
        let note_id = seed_foreign_note(&db, strategy_b, "b's note").await;
        let note_version_id = current_note_version_id(&db, note_id).await;
        let parent_id =
            seed_comment(&db, "note_version", note_version_id, None, "please fix").await;

        let err = server
            .reply_comment_inner(
                strategy_a,
                ReplyCommentParams {
                    parent_id,
                    body: "fixed".into(),
                },
            )
            .await
            .expect_err("cross-strategy parent expected to be rejected");
        assert_eq!(err.code, rmcp::model::ErrorCode::INVALID_PARAMS);
    }

    #[backend_test_macros::database_test]
    async fn reply_comment_rejects_reply_to_reply(db: gateway_postgres::DatabaseHandle) {
        let strategy_id = insert_strategy(&db, "long").await;
        let server = build_server(db.clone());
        let note_id = seed_foreign_note(&db, strategy_id, "note").await;
        let note_version_id = current_note_version_id(&db, note_id).await;
        let root_id = seed_comment(&db, "note_version", note_version_id, None, "please fix").await;
        let reply_id =
            seed_comment(&db, "note_version", note_version_id, Some(root_id), "fixed").await;

        let err = server
            .reply_comment_inner(
                strategy_id,
                ReplyCommentParams {
                    parent_id: reply_id,
                    body: "thanks".into(),
                },
            )
            .await
            .expect_err("reply to a reply expected to be rejected");
        assert_eq!(err.code, rmcp::model::ErrorCode::INVALID_PARAMS);
    }
}
