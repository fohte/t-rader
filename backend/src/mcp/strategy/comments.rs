//! コメント取得の inner method 実装。
//!
//! 戦略境界の二重検査は [`super::ensure_strategy_match`] と、対象 (note /
//! annotation) の所有権検査 ([`super::fetch_note_owned_by`] /
//! [`super::fetch_annotation_owned_by`]) が担う。

use rmcp::ErrorData as McpError;
use sea_orm::ActiveValue::{NotSet, Set};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, IntoActiveModel, QueryFilter, QueryOrder,
};
use uuid::Uuid;

use crate::entities::comment;

use super::dto::{
    CommentDto, ReadCommentsParams, ReadCommentsResult, ReplyCommentParams, ReplyCommentResult,
    ResolveCommentParams, ResolveCommentResult,
};
use super::{
    STRATEGY_AGENT_ACTOR, StrategyServer, db_error, ensure_strategy_match,
    fetch_annotation_owned_by, fetch_note_owned_by, internal_error, invalid_params,
};

const ALLOWED_COMMENT_TARGET_KIND: [&str; 2] = ["note", "annotation"];

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
    }
}

/// comment の target_kind に応じて所有権 (strategy_id 一致) を検査する。
async fn ensure_comment_target_owned_by(
    db: &sea_orm::DatabaseConnection,
    target_kind: &str,
    target_id: Uuid,
    expected: Uuid,
) -> Result<(), McpError> {
    match target_kind {
        "note" => {
            fetch_note_owned_by(db, target_id, expected).await?;
        }
        "annotation" => {
            fetch_annotation_owned_by(db, target_id, expected).await?;
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
        session_strategy_id: Uuid,
        params: ReadCommentsParams,
    ) -> Result<ReadCommentsResult, McpError> {
        ensure_strategy_match(session_strategy_id, params.strategy_id)?;

        match params.target_kind.as_str() {
            "note" => {
                fetch_note_owned_by(&self.db, params.target_id, params.strategy_id).await?;
            }
            "annotation" => {
                fetch_annotation_owned_by(&self.db, params.target_id, params.strategy_id).await?;
            }
            other => {
                return Err(invalid_params(format!(
                    "invalid target_kind: {other} (expected one of {ALLOWED_COMMENT_TARGET_KIND:?})"
                )));
            }
        }

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
        session_strategy_id: Uuid,
        params: ResolveCommentParams,
    ) -> Result<ResolveCommentResult, McpError> {
        ensure_strategy_match(session_strategy_id, params.strategy_id)?;

        let current = comment::Entity::find_by_id(params.comment_id)
            .one(&self.db)
            .await
            .map_err(db_error)?
            .ok_or_else(|| McpError::resource_not_found("comment not found", None))?;
        ensure_comment_target_owned_by(
            &self.db,
            &current.target_kind,
            current.target_id,
            params.strategy_id,
        )
        .await?;

        let mut active = current.into_active_model();
        active.resolved = Set(params.resolved);
        let updated = active.update(&self.db).await.map_err(db_error)?;
        Ok(ResolveCommentResult {
            comment: comment_to_dto(updated),
        })
    }

    pub(crate) async fn reply_comment_inner(
        &self,
        session_strategy_id: Uuid,
        params: ReplyCommentParams,
    ) -> Result<ReplyCommentResult, McpError> {
        ensure_strategy_match(session_strategy_id, params.strategy_id)?;

        if params.body.trim().is_empty() {
            return Err(invalid_params("body must not be empty"));
        }

        let parent = comment::Entity::find_by_id(params.parent_id)
            .one(&self.db)
            .await
            .map_err(db_error)?
            .ok_or_else(|| McpError::resource_not_found("parent comment not found", None))?;
        ensure_comment_target_owned_by(
            &self.db,
            &parent.target_kind,
            parent.target_id,
            params.strategy_id,
        )
        .await?;

        let model = comment::ActiveModel {
            id: Set(Uuid::new_v4()),
            target_kind: Set(parent.target_kind),
            target_id: Set(parent.target_id),
            parent_id: Set(Some(parent.id)),
            body: Set(params.body),
            author_kind: Set(STRATEGY_AGENT_ACTOR.to_string()),
            author_label: Set("analyst".to_string()),
            resolved: Set(false),
            created_at: NotSet,
        };
        let created = comment::Entity::insert(model)
            .exec_with_returning(&self.db)
            .await
            .map_err(db_error)?;
        Ok(ReplyCommentResult {
            comment: comment_to_dto(created),
        })
    }
}

#[cfg(test)]
mod tests {
    use sqlx::PgPool;

    use crate::testing::create_test_db;

    use super::super::dto::{
        CommentDto, ReadCommentsParams, ReplyCommentParams, ResolveCommentParams,
    };
    use super::super::tests_common::{
        build_server, insert_strategy, normalize_comment, seed_comment, seed_foreign_annotation,
        seed_foreign_note, ts_sentinel,
    };

    #[sqlx::test(migrations = false)]
    async fn read_comments_returns_target_comments_in_thread_order(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_id = insert_strategy(&db, "long").await;
        let server = build_server(db.clone());
        let note_id = seed_foreign_note(&db, strategy_id, "note").await;
        let other_note_id = seed_foreign_note(&db, strategy_id, "other note").await;

        let root = seed_comment(&db, "note", note_id, None, "root comment").await;
        let reply = seed_comment(&db, "note", note_id, Some(root), "reply comment").await;
        seed_comment(&db, "note", other_note_id, None, "unrelated comment").await;

        let result = server
            .read_comments_inner(
                strategy_id,
                ReadCommentsParams {
                    strategy_id,
                    target_kind: "note".into(),
                    target_id: note_id,
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
                    target_kind: "note".into(),
                    target_id: note_id,
                    parent_id: None,
                    body: "root comment".into(),
                    author_kind: "human".into(),
                    author_label: "user".into(),
                    resolved: false,
                    created_at: ts_sentinel(),
                },
                CommentDto {
                    comment_id: reply,
                    target_kind: "note".into(),
                    target_id: note_id,
                    parent_id: Some(root),
                    body: "reply comment".into(),
                    author_kind: "human".into(),
                    author_label: "user".into(),
                    resolved: false,
                    created_at: ts_sentinel(),
                },
            ],
        );
    }

    #[sqlx::test(migrations = false)]
    async fn read_comments_supports_annotation_target(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_id = insert_strategy(&db, "swing").await;
        let server = build_server(db.clone());
        let annotation_id = seed_foreign_annotation(&db, strategy_id).await;
        let comment_id = seed_comment(&db, "annotation", annotation_id, None, "looks wrong").await;

        let result = server
            .read_comments_inner(
                strategy_id,
                ReadCommentsParams {
                    strategy_id,
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
            }],
        );
    }

    #[sqlx::test(migrations = false)]
    async fn read_comments_rejects_invalid_target_kind(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_id = insert_strategy(&db, "x").await;
        let server = build_server(db);

        let err = server
            .read_comments_inner(
                strategy_id,
                ReadCommentsParams {
                    strategy_id,
                    target_kind: "garbage".into(),
                    target_id: uuid::Uuid::new_v4(),
                    resolved: None,
                },
            )
            .await
            .expect_err("invalid target_kind expected to be rejected");
        assert_eq!(err.code, rmcp::model::ErrorCode::INVALID_PARAMS);
    }

    #[sqlx::test(migrations = false)]
    async fn read_comments_rejects_cross_strategy_note(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_a = insert_strategy(&db, "a").await;
        let strategy_b = insert_strategy(&db, "b").await;
        let server = build_server(db.clone());
        let note_id = seed_foreign_note(&db, strategy_b, "b's note").await;

        let err = server
            .read_comments_inner(
                strategy_a,
                ReadCommentsParams {
                    strategy_id: strategy_a,
                    target_kind: "note".into(),
                    target_id: note_id,
                    resolved: None,
                },
            )
            .await
            .expect_err("cross-strategy note expected to be rejected");
        assert_eq!(err.code, rmcp::model::ErrorCode::INVALID_PARAMS);
    }

    #[sqlx::test(migrations = false)]
    async fn read_comments_rejects_cross_strategy_annotation(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_a = insert_strategy(&db, "a").await;
        let strategy_b = insert_strategy(&db, "b").await;
        let server = build_server(db.clone());
        let annotation_id = seed_foreign_annotation(&db, strategy_b).await;

        let err = server
            .read_comments_inner(
                strategy_a,
                ReadCommentsParams {
                    strategy_id: strategy_a,
                    target_kind: "annotation".into(),
                    target_id: annotation_id,
                    resolved: None,
                },
            )
            .await
            .expect_err("cross-strategy annotation expected to be rejected");
        assert_eq!(err.code, rmcp::model::ErrorCode::INVALID_PARAMS);
    }

    #[sqlx::test(migrations = false)]
    async fn read_comments_filters_by_resolved(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_id = insert_strategy(&db, "long").await;
        let server = build_server(db.clone());
        let note_id = seed_foreign_note(&db, strategy_id, "note").await;
        let open = seed_comment(&db, "note", note_id, None, "still open").await;
        let done = seed_comment(&db, "note", note_id, None, "already fixed").await;
        server
            .resolve_comment_inner(
                strategy_id,
                ResolveCommentParams {
                    strategy_id,
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
                    strategy_id,
                    target_kind: "note".into(),
                    target_id: note_id,
                    resolved: Some(false),
                },
            )
            .await
            .expect("read_comments");
        assert_eq!(
            unresolved_only
                .comments
                .into_iter()
                .map(|c| c.comment_id)
                .collect::<Vec<_>>(),
            vec![open],
        );

        let resolved_only = server
            .read_comments_inner(
                strategy_id,
                ReadCommentsParams {
                    strategy_id,
                    target_kind: "note".into(),
                    target_id: note_id,
                    resolved: Some(true),
                },
            )
            .await
            .expect("read_comments");
        assert_eq!(
            resolved_only
                .comments
                .into_iter()
                .map(|c| c.comment_id)
                .collect::<Vec<_>>(),
            vec![done],
        );
    }

    #[sqlx::test(migrations = false)]
    async fn resolve_comment_toggles_resolved(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_id = insert_strategy(&db, "long").await;
        let server = build_server(db.clone());
        let note_id = seed_foreign_note(&db, strategy_id, "note").await;
        let comment_id = seed_comment(&db, "note", note_id, None, "fix this").await;

        let result = server
            .resolve_comment_inner(
                strategy_id,
                ResolveCommentParams {
                    strategy_id,
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
                target_kind: "note".into(),
                target_id: note_id,
                parent_id: None,
                body: "fix this".into(),
                author_kind: "human".into(),
                author_label: "user".into(),
                resolved: true,
                created_at: ts_sentinel(),
            },
        );

        let result = server
            .resolve_comment_inner(
                strategy_id,
                ResolveCommentParams {
                    strategy_id,
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
                target_kind: "note".into(),
                target_id: note_id,
                parent_id: None,
                body: "fix this".into(),
                author_kind: "human".into(),
                author_label: "user".into(),
                resolved: false,
                created_at: ts_sentinel(),
            },
        );
    }

    #[sqlx::test(migrations = false)]
    async fn resolve_comment_rejects_missing_comment(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_id = insert_strategy(&db, "x").await;
        let server = build_server(db);

        let err = server
            .resolve_comment_inner(
                strategy_id,
                ResolveCommentParams {
                    strategy_id,
                    comment_id: uuid::Uuid::new_v4(),
                    resolved: true,
                },
            )
            .await
            .expect_err("missing comment expected to be rejected");
        assert_eq!(err.code, rmcp::model::ErrorCode::RESOURCE_NOT_FOUND);
    }

    #[sqlx::test(migrations = false)]
    async fn resolve_comment_rejects_cross_strategy(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_a = insert_strategy(&db, "a").await;
        let strategy_b = insert_strategy(&db, "b").await;
        let server = build_server(db.clone());
        let note_id = seed_foreign_note(&db, strategy_b, "b's note").await;
        let comment_id = seed_comment(&db, "note", note_id, None, "fix this").await;

        let err = server
            .resolve_comment_inner(
                strategy_a,
                ResolveCommentParams {
                    strategy_id: strategy_a,
                    comment_id,
                    resolved: true,
                },
            )
            .await
            .expect_err("cross-strategy comment expected to be rejected");
        assert_eq!(err.code, rmcp::model::ErrorCode::INVALID_PARAMS);
    }

    #[sqlx::test(migrations = false)]
    async fn reply_comment_inherits_parent_target(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_id = insert_strategy(&db, "long").await;
        let server = build_server(db.clone());
        let note_id = seed_foreign_note(&db, strategy_id, "note").await;
        let parent_id = seed_comment(&db, "note", note_id, None, "please fix").await;

        let result = server
            .reply_comment_inner(
                strategy_id,
                ReplyCommentParams {
                    strategy_id,
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
                target_kind: "note".into(),
                target_id: note_id,
                parent_id: Some(parent_id),
                body: "fixed in the latest revision".into(),
                author_kind: super::super::STRATEGY_AGENT_ACTOR.into(),
                author_label: "analyst".into(),
                resolved: false,
                created_at: ts_sentinel(),
            },
        );
    }

    #[sqlx::test(migrations = false)]
    async fn reply_comment_rejects_empty_body(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_id = insert_strategy(&db, "long").await;
        let server = build_server(db.clone());
        let note_id = seed_foreign_note(&db, strategy_id, "note").await;
        let parent_id = seed_comment(&db, "note", note_id, None, "please fix").await;

        let err = server
            .reply_comment_inner(
                strategy_id,
                ReplyCommentParams {
                    strategy_id,
                    parent_id,
                    body: "   ".into(),
                },
            )
            .await
            .expect_err("empty body expected to be rejected");
        assert_eq!(err.code, rmcp::model::ErrorCode::INVALID_PARAMS);
    }

    #[sqlx::test(migrations = false)]
    async fn reply_comment_rejects_missing_parent(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_id = insert_strategy(&db, "x").await;
        let server = build_server(db);

        let err = server
            .reply_comment_inner(
                strategy_id,
                ReplyCommentParams {
                    strategy_id,
                    parent_id: uuid::Uuid::new_v4(),
                    body: "fixed".into(),
                },
            )
            .await
            .expect_err("missing parent expected to be rejected");
        assert_eq!(err.code, rmcp::model::ErrorCode::RESOURCE_NOT_FOUND);
    }

    #[sqlx::test(migrations = false)]
    async fn reply_comment_rejects_cross_strategy(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_a = insert_strategy(&db, "a").await;
        let strategy_b = insert_strategy(&db, "b").await;
        let server = build_server(db.clone());
        let note_id = seed_foreign_note(&db, strategy_b, "b's note").await;
        let parent_id = seed_comment(&db, "note", note_id, None, "please fix").await;

        let err = server
            .reply_comment_inner(
                strategy_a,
                ReplyCommentParams {
                    strategy_id: strategy_a,
                    parent_id,
                    body: "fixed".into(),
                },
            )
            .await
            .expect_err("cross-strategy parent expected to be rejected");
        assert_eq!(err.code, rmcp::model::ErrorCode::INVALID_PARAMS);
    }
}
