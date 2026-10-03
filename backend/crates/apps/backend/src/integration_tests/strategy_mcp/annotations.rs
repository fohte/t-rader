#[cfg(test)]
mod tests {
    use chrono::{DateTime, FixedOffset};
    use sea_orm::ActiveModelTrait;
    use sea_orm::ActiveValue::Set;
    use uuid::Uuid;

    use super::super::dto::{
        AnnotationDto, CreateAnnotationParams, CreateAnnotationResult, ReadAnnotationsParams,
        ReadAnnotationsResult,
    };
    use super::super::tests_common::{
        ChangeHistoryShape, build_server, change_history_for, insert_strategy,
        normalize_annotation, normalize_create_annotation, normalize_read_annotations,
        normalize_read_annotations_unordered, seed_annotation, seed_comment, seed_note,
        ts_sentinel,
    };
    use super::super::{DEFAULT_ANNOTATION_STATUS, STRATEGY_AGENT_ACTOR};
    use gateway_postgres::entities::annotation;

    // target_kind に旧 allowlist 外の値を使い、DB の CHECK 制約撤去 (target_kind は自由記述) を回帰検出する
    #[backend_test_macros::database_test]
    async fn create_annotation_then_read_annotations(db: gateway_postgres::DatabaseHandle) {
        let strategy_id = insert_strategy(&db, "swing").await;
        let server = build_server(db.clone());
        let ts: DateTime<FixedOffset> = "2026-06-01T09:00:00+09:00".parse().expect("ts");

        let created = server
            .create_annotation(
                strategy_id,
                None,
                None,
                CreateAnnotationParams {
                    target_symbol: "7203".into(),
                    target_kind: "custom-tag".into(),
                    timestamp: ts,
                    price: Some(25000.0),
                    text: "breakout".into(),
                    linked_note_id: None,
                },
            )
            .await
            .expect("create");
        let annotation_id = created.annotation.annotation_id;
        let expected = AnnotationDto {
            annotation_id,
            target_symbol: "7203".into(),
            target_kind: "custom-tag".into(),
            timestamp: ts.with_timezone(&chrono::Utc).fixed_offset(),
            price: Some(25000.0),
            text: "breakout".into(),
            status: DEFAULT_ANNOTATION_STATUS.into(),
            linked_note_id: None,
            created_by_kind: STRATEGY_AGENT_ACTOR.into(),
            created_at: ts_sentinel(),
            updated_at: ts_sentinel(),
        };
        assert_eq!(
            normalize_create_annotation(created),
            CreateAnnotationResult {
                annotation: expected.clone(),
            },
        );

        let list = server
            .read_annotations(
                strategy_id,
                ReadAnnotationsParams {
                    target_symbol: None,
                    limit: None,
                },
            )
            .await
            .expect("list");
        assert_eq!(
            normalize_read_annotations(list),
            ReadAnnotationsResult {
                annotations: vec![expected],
            },
        );
        assert_eq!(
            change_history_for(&db, annotation_id).await,
            vec![ChangeHistoryShape {
                id: Uuid::nil(),
                target_kind: "annotation".into(),
                target_id: annotation_id,
                actor_kind: "llm".into(),
                actor_label: "analyst".into(),
                op: "create".into(),
                diff_json: serde_json::json!({
                    "target_symbol": "7203",
                }),
                summary: None,
                created_at: ts_sentinel(),
            }],
        );
    }

    #[backend_test_macros::database_test]
    async fn read_annotations_returns_globally_stored_annotations(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let strategy_id = insert_strategy(&db, "swing").await;
        let annotation_id = seed_annotation(&db).await;
        let server = build_server(db);
        let timestamp: DateTime<FixedOffset> = "2026-06-01T00:00:00Z".parse().expect("ts");

        let result = server
            .read_annotations(
                strategy_id,
                ReadAnnotationsParams {
                    target_symbol: None,
                    limit: None,
                },
            )
            .await
            .expect("read annotations without a strategy");
        assert_eq!(
            normalize_read_annotations(result),
            ReadAnnotationsResult {
                annotations: vec![AnnotationDto {
                    annotation_id,
                    target_symbol: "demo-code".into(),
                    target_kind: "sample-tag".into(),
                    timestamp: timestamp.with_timezone(&chrono::Utc).fixed_offset(),
                    price: None,
                    text: "breakout".into(),
                    status: DEFAULT_ANNOTATION_STATUS.into(),
                    linked_note_id: None,
                    created_by_kind: STRATEGY_AGENT_ACTOR.into(),
                    created_at: ts_sentinel(),
                    updated_at: ts_sentinel(),
                }],
            },
        );
    }

    #[backend_test_macros::database_test]
    async fn create_annotation_rejects_empty_target_kind(db: gateway_postgres::DatabaseHandle) {
        let strategy_id = insert_strategy(&db, "x").await;
        let server = build_server(db);
        let err = server
            .create_annotation(
                strategy_id,
                None,
                None,
                CreateAnnotationParams {
                    target_symbol: "7203".into(),
                    target_kind: "  ".into(),
                    timestamp: "2026-06-01T00:00:00Z".parse().expect("ts"),
                    price: None,
                    text: "x".into(),
                    linked_note_id: None,
                },
            )
            .await
            .expect_err("empty target_kind expected to be rejected");
        assert_eq!(
            err,
            rmcp::ErrorData::invalid_params("target_kind must not be empty", None),
        );
    }

    #[backend_test_macros::database_test]
    async fn create_annotation_links_a_note_without_strategy_ownership(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let strategy_a = insert_strategy(&db, "a").await;
        let server = build_server(db.clone());
        let foreign_note = seed_note(&db, "sample note").await;

        let timestamp = "2026-06-01T00:00:00Z".parse().expect("ts");
        let result = server
            .create_annotation(
                strategy_a,
                None,
                None,
                CreateAnnotationParams {
                    target_symbol: "7203".into(),
                    target_kind: "custom-tag".into(),
                    timestamp,
                    price: None,
                    text: "x".into(),
                    linked_note_id: Some(foreign_note),
                },
            )
            .await
            .expect("link note globally");
        let annotation_id = result.annotation.annotation_id;
        assert_eq!(
            normalize_create_annotation(result),
            CreateAnnotationResult {
                annotation: AnnotationDto {
                    annotation_id,
                    target_symbol: "7203".into(),
                    target_kind: "custom-tag".into(),
                    timestamp: timestamp.with_timezone(&chrono::Utc).fixed_offset(),
                    price: None,
                    text: "x".into(),
                    status: DEFAULT_ANNOTATION_STATUS.into(),
                    linked_note_id: Some(foreign_note),
                    created_by_kind: STRATEGY_AGENT_ACTOR.into(),
                    created_at: ts_sentinel(),
                    updated_at: ts_sentinel(),
                },
            },
        );
    }

    /// resume で同じステップの新しい試行 (execution_task_id が変わる) が create_annotation を
    /// 呼んだとき、前の試行が作った未レビュー (unread) のアノテーションは削除され、
    /// 新しい試行のものだけが残る。
    #[backend_test_macros::database_test]
    async fn create_annotation_replaces_unread_annotations_from_previous_attempt_of_same_step(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let strategy_id = insert_strategy(&db, "swing").await;
        let server = build_server(db);
        let step_id = Uuid::new_v4();
        let ts: DateTime<FixedOffset> = "2026-06-01T00:00:00Z".parse().expect("ts");

        server
            .create_annotation(
                strategy_id,
                Some(step_id),
                Some("attempt-1".into()),
                CreateAnnotationParams {
                    target_symbol: "7203".into(),
                    target_kind: "signal".into(),
                    timestamp: ts,
                    price: None,
                    text: "first attempt".into(),
                    linked_note_id: None,
                },
            )
            .await
            .expect("create from first attempt");

        let second = server
            .create_annotation(
                strategy_id,
                Some(step_id),
                Some("attempt-2".into()),
                CreateAnnotationParams {
                    target_symbol: "7203".into(),
                    target_kind: "signal".into(),
                    timestamp: ts,
                    price: None,
                    text: "second attempt".into(),
                    linked_note_id: None,
                },
            )
            .await
            .expect("create from second (resumed) attempt");

        let list = server
            .read_annotations(
                strategy_id,
                ReadAnnotationsParams {
                    target_symbol: None,
                    limit: None,
                },
            )
            .await
            .expect("list");
        assert_eq!(
            normalize_read_annotations(list),
            ReadAnnotationsResult {
                annotations: vec![normalize_annotation(second.annotation.clone())],
            },
        );
    }

    /// 前の試行が作ったアノテーションでも、既にレビュー済み (unread 以外) のものは
    /// 新しい試行が来ても削除されず残る。
    #[backend_test_macros::database_test]
    async fn create_annotation_keeps_reviewed_annotations_from_previous_attempt(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let strategy_id = insert_strategy(&db, "swing").await;
        let server = build_server(db.clone());
        let step_id = Uuid::new_v4();
        let ts: DateTime<FixedOffset> = "2026-06-01T00:00:00Z".parse().expect("ts");

        let reviewed = server
            .create_annotation(
                strategy_id,
                Some(step_id),
                Some("attempt-1".into()),
                CreateAnnotationParams {
                    target_symbol: "7203".into(),
                    target_kind: "signal".into(),
                    timestamp: ts,
                    price: None,
                    text: "already reviewed".into(),
                    linked_note_id: None,
                },
            )
            .await
            .expect("create from first attempt");
        annotation::ActiveModel {
            id: Set(reviewed.annotation.annotation_id),
            status: Set("approved".into()),
            ..Default::default()
        }
        .update(&db)
        .await
        .expect("mark approved");

        let second = server
            .create_annotation(
                strategy_id,
                Some(step_id),
                Some("attempt-2".into()),
                CreateAnnotationParams {
                    target_symbol: "7203".into(),
                    target_kind: "signal".into(),
                    timestamp: ts,
                    price: None,
                    text: "second attempt".into(),
                    linked_note_id: None,
                },
            )
            .await
            .expect("create from second (resumed) attempt");

        let list = server
            .read_annotations(
                strategy_id,
                ReadAnnotationsParams {
                    target_symbol: None,
                    limit: None,
                },
            )
            .await
            .expect("list");
        let mut reviewed_annotation = reviewed.annotation.clone();
        reviewed_annotation.status = "approved".into();
        let mut expected = vec![
            normalize_annotation(reviewed_annotation),
            normalize_annotation(second.annotation.clone()),
        ];
        expected.sort_by_key(|annotation| annotation.annotation_id);
        assert_eq!(
            normalize_read_annotations_unordered(list),
            ReadAnnotationsResult {
                annotations: expected,
            },
        );
    }

    /// 前の試行が作った unread のアノテーションでも、既にコメントが付いている場合は
    /// (comment.target_id が FK を持たないため) 削除すると孤児化してしまうので残る。
    #[backend_test_macros::database_test]
    async fn create_annotation_keeps_unread_annotations_with_comments_from_previous_attempt(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let strategy_id = insert_strategy(&db, "swing").await;
        let server = build_server(db.clone());
        let step_id = Uuid::new_v4();
        let ts: DateTime<FixedOffset> = "2026-06-01T00:00:00Z".parse().expect("ts");

        let commented = server
            .create_annotation(
                strategy_id,
                Some(step_id),
                Some("attempt-1".into()),
                CreateAnnotationParams {
                    target_symbol: "7203".into(),
                    target_kind: "signal".into(),
                    timestamp: ts,
                    price: None,
                    text: "commented but unread".into(),
                    linked_note_id: None,
                },
            )
            .await
            .expect("create from first attempt");
        seed_comment(
            &db,
            "annotation",
            commented.annotation.annotation_id,
            None,
            "why?",
        )
        .await;

        let second = server
            .create_annotation(
                strategy_id,
                Some(step_id),
                Some("attempt-2".into()),
                CreateAnnotationParams {
                    target_symbol: "7203".into(),
                    target_kind: "signal".into(),
                    timestamp: ts,
                    price: None,
                    text: "second attempt".into(),
                    linked_note_id: None,
                },
            )
            .await
            .expect("create from second (resumed) attempt");

        let list = server
            .read_annotations(
                strategy_id,
                ReadAnnotationsParams {
                    target_symbol: None,
                    limit: None,
                },
            )
            .await
            .expect("list");
        let mut expected = vec![
            normalize_annotation(commented.annotation.clone()),
            normalize_annotation(second.annotation.clone()),
        ];
        expected.sort_by_key(|annotation| annotation.annotation_id);
        assert_eq!(
            normalize_read_annotations_unordered(list),
            ReadAnnotationsResult {
                annotations: expected,
            },
        );
    }

    /// resume していない通常の実行 (同じ execution_task_id) で 1 ステップが複数件の
    /// アノテーションを作る動作はこれまでどおり全件残る。
    #[backend_test_macros::database_test]
    async fn create_annotation_keeps_multiple_annotations_from_the_same_attempt(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let strategy_id = insert_strategy(&db, "swing").await;
        let server = build_server(db);
        let step_id = Uuid::new_v4();
        let ts: DateTime<FixedOffset> = "2026-06-01T00:00:00Z".parse().expect("ts");

        let mut expected = Vec::new();
        for text in ["first", "second"] {
            let created = server
                .create_annotation(
                    strategy_id,
                    Some(step_id),
                    Some("attempt-1".into()),
                    CreateAnnotationParams {
                        target_symbol: "7203".into(),
                        target_kind: "signal".into(),
                        timestamp: ts,
                        price: None,
                        text: text.into(),
                        linked_note_id: None,
                    },
                )
                .await
                .unwrap_or_else(|e| panic!("create {text} failed: {e}"));
            expected.push(normalize_annotation(created.annotation.clone()));
        }
        expected.sort_by_key(|annotation| annotation.annotation_id);

        let list = server
            .read_annotations(
                strategy_id,
                ReadAnnotationsParams {
                    target_symbol: None,
                    limit: None,
                },
            )
            .await
            .expect("list");
        assert_eq!(
            normalize_read_annotations_unordered(list),
            ReadAnnotationsResult {
                annotations: expected,
            },
        );
    }
}
