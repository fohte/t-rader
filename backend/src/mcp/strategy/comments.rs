//! コメント取得の inner method 実装。
//!
//! 戦略境界の二重検査は [`super::ensure_strategy_match`] と、対象 (note /
//! annotation) の所有権検査 ([`super::fetch_note_owned_by`] /
//! [`super::fetch_annotation_owned_by`]) が担う。

use rmcp::ErrorData as McpError;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
use uuid::Uuid;

use crate::entities::comment;

use super::dto::{CommentDto, ReadCommentsParams, ReadCommentsResult};
use super::{
    StrategyServer, db_error, ensure_strategy_match, fetch_annotation_owned_by,
    fetch_note_owned_by, invalid_params,
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
        created_at: m.created_at,
    }
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

        let rows = comment::Entity::find()
            .filter(comment::Column::TargetKind.eq(params.target_kind))
            .filter(comment::Column::TargetId.eq(params.target_id))
            .order_by_asc(comment::Column::CreatedAt)
            .all(&self.db)
            .await
            .map_err(db_error)?;
        Ok(ReadCommentsResult {
            comments: rows.into_iter().map(comment_to_dto).collect(),
        })
    }
}

#[cfg(test)]
mod tests {
    use sqlx::PgPool;

    use crate::testing::create_test_db;

    use super::super::dto::{CreateAnnotationParams, ReadCommentsParams};
    use super::super::tests_common::{
        build_server, insert_strategy, seed_comment, seed_foreign_note,
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
                },
            )
            .await
            .expect("read_comments");

        let got: Vec<(uuid::Uuid, Option<uuid::Uuid>, &str)> = result
            .comments
            .iter()
            .map(|c| (c.comment_id, c.parent_id, c.body.as_str()))
            .collect();
        assert_eq!(
            got,
            vec![
                (root, None, "root comment"),
                (reply, Some(root), "reply comment"),
            ],
        );
    }

    #[sqlx::test(migrations = false)]
    async fn read_comments_supports_annotation_target(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_id = insert_strategy(&db, "swing").await;
        let server = build_server(db.clone());
        let created = server
            .create_annotation_inner(
                strategy_id,
                CreateAnnotationParams {
                    strategy_id,
                    target_symbol: "7203".into(),
                    target_kind: "signal".into(),
                    timestamp: "2026-06-01T00:00:00Z".parse().expect("ts"),
                    price: None,
                    text: "breakout".into(),
                    linked_note_id: None,
                },
            )
            .await
            .expect("create annotation");
        let annotation_id = created.annotation.annotation_id;
        seed_comment(&db, "annotation", annotation_id, None, "looks wrong").await;

        let result = server
            .read_comments_inner(
                strategy_id,
                ReadCommentsParams {
                    strategy_id,
                    target_kind: "annotation".into(),
                    target_id: annotation_id,
                },
            )
            .await
            .expect("read_comments");
        let bodies: Vec<&str> = result.comments.iter().map(|c| c.body.as_str()).collect();
        assert_eq!(bodies, vec!["looks wrong"]);
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
        let created = server
            .create_annotation_inner(
                strategy_b,
                CreateAnnotationParams {
                    strategy_id: strategy_b,
                    target_symbol: "7203".into(),
                    target_kind: "signal".into(),
                    timestamp: "2026-06-01T00:00:00Z".parse().expect("ts"),
                    price: None,
                    text: "breakout".into(),
                    linked_note_id: None,
                },
            )
            .await
            .expect("create annotation");

        let err = server
            .read_comments_inner(
                strategy_a,
                ReadCommentsParams {
                    strategy_id: strategy_a,
                    target_kind: "annotation".into(),
                    target_id: created.annotation.annotation_id,
                },
            )
            .await
            .expect_err("cross-strategy annotation expected to be rejected");
        assert_eq!(err.code, rmcp::model::ErrorCode::INVALID_PARAMS);
    }
}
