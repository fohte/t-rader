#[cfg(test)]
mod tests {
    use super::super::dto::{
        CommentDto, ReadCommentsParams, ReadCommentsResult, ReplyCommentParams, ReplyCommentResult,
        ResolveCommentParams, ResolveCommentResult,
    };
    use super::super::tests_common::{
        ChangeHistoryShape, build_server, change_history_for, current_note_version_id,
        insert_strategy, normalize_read_comments, normalize_reply_comment,
        normalize_resolve_comment, seed_annotation, seed_comment, seed_note, ts_sentinel,
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
        let note_id = seed_note(&db, "sample note").await;
        let other_note_id = seed_note(&db, "another sample note").await;
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
            .read_comments(
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
            normalize_read_comments(result),
            ReadCommentsResult {
                comments: vec![
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
            },
        );
    }

    #[backend_test_macros::database_test]
    async fn read_comments_supports_annotation_target(db: gateway_postgres::DatabaseHandle) {
        let strategy_id = insert_strategy(&db, "swing").await;
        let server = build_server(db.clone());
        let annotation_id = seed_annotation(&db).await;
        let comment_id = seed_comment(&db, "annotation", annotation_id, None, "looks wrong").await;

        let result = server
            .read_comments(
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
            normalize_read_comments(result),
            ReadCommentsResult {
                comments: vec![CommentDto {
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
            },
        );
    }

    #[backend_test_macros::database_test]
    async fn read_comments_preserves_missing_annotation_error(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let strategy_id = insert_strategy(&db, "swing").await;
        let server = build_server(db);

        let error = server
            .read_comments(
                strategy_id,
                ReadCommentsParams {
                    target_kind: "annotation".into(),
                    target_id: uuid::Uuid::new_v4(),
                    resolved: None,
                },
            )
            .await
            .expect_err("missing annotation expected to be rejected");

        assert_eq!(
            error,
            rmcp::ErrorData::resource_not_found("annotation not found", None),
        );
    }

    #[backend_test_macros::database_test]
    async fn read_comments_rejects_invalid_target_kind(db: gateway_postgres::DatabaseHandle) {
        let strategy_id = insert_strategy(&db, "x").await;
        let server = build_server(db);

        let err = server
            .read_comments(
                strategy_id,
                ReadCommentsParams {
                    target_kind: "garbage".into(),
                    target_id: uuid::Uuid::new_v4(),
                    resolved: None,
                },
            )
            .await
            .expect_err("invalid target_kind expected to be rejected");
        assert_eq!(
            err,
            rmcp::ErrorData::invalid_params(
                "invalid target_kind: garbage (expected one of [\"note_version\", \"annotation\"])",
                None,
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn read_comments_returns_comments_for_a_global_note(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let strategy_a = insert_strategy(&db, "a").await;
        let server = build_server(db.clone());
        let note_id = seed_note(&db, "sample note").await;
        let note_version_id = current_note_version_id(&db, note_id).await;
        let comment_id =
            seed_comment(&db, "note_version", note_version_id, None, "review this").await;

        let result = server
            .read_comments(
                strategy_a,
                ReadCommentsParams {
                    target_kind: "note_version".into(),
                    target_id: note_version_id,
                    resolved: None,
                },
            )
            .await
            .expect("read comments on a note from another strategy");
        assert_eq!(
            normalize_read_comments(result),
            ReadCommentsResult {
                comments: vec![CommentDto {
                    comment_id,
                    target_kind: "note_version".into(),
                    target_id: note_version_id,
                    parent_id: None,
                    body: "review this".into(),
                    author_kind: "human".into(),
                    author_label: "user".into(),
                    resolved: false,
                    created_at: ts_sentinel(),
                    anchor_text: None,
                    anchor_side: None,
                    start_line: None,
                    end_line: None,
                }],
            },
        );
    }

    #[backend_test_macros::database_test]
    async fn read_comments_returns_comments_for_a_global_annotation(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let strategy_a = insert_strategy(&db, "a").await;
        let server = build_server(db.clone());
        let annotation_id = seed_annotation(&db).await;
        let comment_id = seed_comment(&db, "annotation", annotation_id, None, "review this").await;

        let result = server
            .read_comments(
                strategy_a,
                ReadCommentsParams {
                    target_kind: "annotation".into(),
                    target_id: annotation_id,
                    resolved: None,
                },
            )
            .await
            .expect("read comments on an annotation from another strategy");
        assert_eq!(
            normalize_read_comments(result),
            ReadCommentsResult {
                comments: vec![CommentDto {
                    comment_id,
                    target_kind: "annotation".into(),
                    target_id: annotation_id,
                    parent_id: None,
                    body: "review this".into(),
                    author_kind: "human".into(),
                    author_label: "user".into(),
                    resolved: false,
                    created_at: ts_sentinel(),
                    anchor_text: None,
                    anchor_side: None,
                    start_line: None,
                    end_line: None,
                }],
            },
        );
    }

    #[backend_test_macros::database_test]
    async fn read_comments_filters_by_resolved(db: gateway_postgres::DatabaseHandle) {
        let strategy_id = insert_strategy(&db, "long").await;
        let server = build_server(db.clone());
        let note_id = seed_note(&db, "sample note").await;
        let note_version_id = current_note_version_id(&db, note_id).await;
        let open = seed_comment(&db, "note_version", note_version_id, None, "still open").await;
        let done = seed_comment(&db, "note_version", note_version_id, None, "already fixed").await;
        server
            .resolve_comment(
                strategy_id,
                ResolveCommentParams {
                    comment_id: done,
                    resolved: true,
                },
            )
            .await
            .expect("resolve_comment");

        let unresolved_only = server
            .read_comments(
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
            normalize_read_comments(unresolved_only),
            ReadCommentsResult {
                comments: vec![CommentDto {
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
            },
        );

        let resolved_only = server
            .read_comments(
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
            normalize_read_comments(resolved_only),
            ReadCommentsResult {
                comments: vec![CommentDto {
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
            },
        );
    }

    #[backend_test_macros::database_test]
    async fn resolve_comment_toggles_resolved(db: gateway_postgres::DatabaseHandle) {
        let strategy_id = insert_strategy(&db, "long").await;
        let server = build_server(db.clone());
        let note_id = seed_note(&db, "sample note").await;
        let note_version_id = current_note_version_id(&db, note_id).await;
        let comment_id = seed_comment(&db, "note_version", note_version_id, None, "fix this").await;

        let result = server
            .resolve_comment(
                strategy_id,
                ResolveCommentParams {
                    comment_id,
                    resolved: true,
                },
            )
            .await
            .expect("resolve_comment");
        assert_eq!(
            normalize_resolve_comment(result),
            ResolveCommentResult {
                comment: CommentDto {
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
            },
        );

        server
            .resolve_comment(
                strategy_id,
                ResolveCommentParams {
                    comment_id,
                    resolved: true,
                },
            )
            .await
            .expect("resolving an already resolved comment is a no-op");

        let result = server
            .resolve_comment(
                strategy_id,
                ResolveCommentParams {
                    comment_id,
                    resolved: false,
                },
            )
            .await
            .expect("resolve_comment");
        assert_eq!(
            normalize_resolve_comment(result),
            ResolveCommentResult {
                comment: CommentDto {
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
            },
        );
        let mut history = change_history_for(&db, comment_id).await;
        let mut expected_history = vec![
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
        ];
        // CURRENT_TIMESTAMP は transaction 内で同じ値になるため、同時刻の行順は DB で確定しない。
        history.sort_by_key(|entry| entry.diff_json.to_string());
        expected_history.sort_by_key(|entry| entry.diff_json.to_string());
        assert_eq!(history, expected_history);
    }

    #[backend_test_macros::database_test]
    async fn resolve_comment_rejects_missing_comment(db: gateway_postgres::DatabaseHandle) {
        let strategy_id = insert_strategy(&db, "x").await;
        let server = build_server(db);
        let comment_id = uuid::Uuid::new_v4();

        let err = server
            .resolve_comment(
                strategy_id,
                ResolveCommentParams {
                    comment_id,
                    resolved: true,
                },
            )
            .await
            .expect_err("missing comment expected to be rejected");
        assert_eq!(
            err,
            rmcp::ErrorData::resource_not_found(format!("comment {comment_id} not found"), None,),
        );
    }

    #[backend_test_macros::database_test]
    async fn resolve_comment_resolves_comment_on_a_global_note(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let strategy_a = insert_strategy(&db, "a").await;
        let server = build_server(db.clone());
        let note_id = seed_note(&db, "sample note").await;
        let note_version_id = current_note_version_id(&db, note_id).await;
        let comment_id = seed_comment(&db, "note_version", note_version_id, None, "fix this").await;

        let result = server
            .resolve_comment(
                strategy_a,
                ResolveCommentParams {
                    comment_id,
                    resolved: true,
                },
            )
            .await
            .expect("resolve comment on note from another strategy");
        assert_eq!(
            normalize_resolve_comment(result),
            ResolveCommentResult {
                comment: CommentDto {
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
            },
        );
    }

    #[backend_test_macros::database_test]
    async fn reply_comment_inherits_parent_target(db: gateway_postgres::DatabaseHandle) {
        let strategy_id = insert_strategy(&db, "long").await;
        let server = build_server(db.clone());
        let note_id = seed_note(&db, "sample note").await;
        let note_version_id = current_note_version_id(&db, note_id).await;
        let parent_id =
            seed_comment(&db, "note_version", note_version_id, None, "please fix").await;

        let result = server
            .reply_comment(
                strategy_id,
                ReplyCommentParams {
                    parent_id,
                    body: "fixed in the latest revision".into(),
                },
            )
            .await
            .expect("reply_comment");

        let comment_id = result.comment.comment_id;
        assert_eq!(
            normalize_reply_comment(result),
            ReplyCommentResult {
                comment: CommentDto {
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
        let note_id = seed_note(&db, "sample note").await;
        let note_version_id = current_note_version_id(&db, note_id).await;
        let parent_id =
            seed_comment(&db, "note_version", note_version_id, None, "please fix").await;

        let err = server
            .reply_comment(
                strategy_id,
                ReplyCommentParams {
                    parent_id,
                    body: "   ".into(),
                },
            )
            .await
            .expect_err("empty body expected to be rejected");
        assert_eq!(
            err,
            rmcp::ErrorData::invalid_params("body must not be empty", None),
        );
    }

    #[backend_test_macros::database_test]
    async fn reply_comment_rejects_missing_parent(db: gateway_postgres::DatabaseHandle) {
        let strategy_id = insert_strategy(&db, "x").await;
        let server = build_server(db);
        let parent_id = uuid::Uuid::new_v4();

        let err = server
            .reply_comment(
                strategy_id,
                ReplyCommentParams {
                    parent_id,
                    body: "fixed".into(),
                },
            )
            .await
            .expect_err("missing parent expected to be rejected");
        assert_eq!(
            err,
            rmcp::ErrorData::resource_not_found(
                format!("parent comment {parent_id} not found"),
                None,
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn reply_comment_replies_to_a_global_note(db: gateway_postgres::DatabaseHandle) {
        let strategy_a = insert_strategy(&db, "a").await;
        let server = build_server(db.clone());
        let note_id = seed_note(&db, "sample note").await;
        let note_version_id = current_note_version_id(&db, note_id).await;
        let parent_id =
            seed_comment(&db, "note_version", note_version_id, None, "please fix").await;

        let result = server
            .reply_comment(
                strategy_a,
                ReplyCommentParams {
                    parent_id,
                    body: "fixed".into(),
                },
            )
            .await
            .expect("reply to comment on a note from another strategy");
        let comment_id = result.comment.comment_id;
        let change_history = change_history_for(&db, comment_id).await;
        let normalized_reply = normalize_reply_comment(result);
        assert_eq!(
            (normalized_reply.as_json().clone(), change_history),
            (
                serde_json::to_value(ReplyCommentResult {
                    comment: CommentDto {
                        comment_id,
                        target_kind: "note_version".into(),
                        target_id: note_version_id,
                        parent_id: Some(parent_id),
                        body: "fixed".into(),
                        author_kind: super::super::STRATEGY_AGENT_ACTOR.into(),
                        author_label: "analyst".into(),
                        resolved: false,
                        created_at: ts_sentinel(),
                        anchor_text: None,
                        anchor_side: None,
                        start_line: None,
                        end_line: None,
                    },
                })
                .expect("serialize expected reply"),
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
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn reply_comment_rejects_reply_to_reply(db: gateway_postgres::DatabaseHandle) {
        let strategy_id = insert_strategy(&db, "long").await;
        let server = build_server(db.clone());
        let note_id = seed_note(&db, "sample note").await;
        let note_version_id = current_note_version_id(&db, note_id).await;
        let root_id = seed_comment(&db, "note_version", note_version_id, None, "please fix").await;
        let reply_id =
            seed_comment(&db, "note_version", note_version_id, Some(root_id), "fixed").await;

        let err = server
            .reply_comment(
                strategy_id,
                ReplyCommentParams {
                    parent_id: reply_id,
                    body: "thanks".into(),
                },
            )
            .await
            .expect_err("reply to a reply expected to be rejected");
        assert_eq!(
            err,
            rmcp::ErrorData::invalid_params(
                "cannot reply to a reply; parent_id must reference a top-level comment",
                None,
            ),
        );
    }
}
