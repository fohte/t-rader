#[cfg(test)]
mod tests {
    use super::super::{TaskShape, assert_response_eq, normalize_annotation_response};
    use std::sync::Arc;

    use crate::testing::agent_config;
    use crate::testing::{
        create_test_server_with_db, create_test_server_with_db_and_agent_client, insert_test_note,
        insert_test_strategy, insert_test_strategy_task_step,
        set_test_annotation_execution_step_id,
    };
    use axum::http::StatusCode;
    use axum_test::TestServer;
    use core_application::agent_task_client::{
        AgentTaskError, FakeAgentTaskClient, SharedAgentTaskClient,
    };
    use core_application::strategy_task::DEFAULT_PURPOSE;
    use gateway_postgres::entities::annotation;
    use gateway_postgres::entities::sea_orm_active_enums::StrategyTaskPhase;
    use gateway_postgres::entities::strategy_task;
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
    use serde_json::{Value, json};
    use uuid::Uuid;

    async fn create_test_annotation(server: &TestServer) -> Uuid {
        create_test_annotation_with_symbol(server, "sample-code").await
    }

    async fn create_test_annotation_with_symbol(server: &TestServer, target_symbol: &str) -> Uuid {
        let res = server
            .post("/api/annotations")
            .json(&json!({
                "target_symbol": target_symbol,
                "target_kind": "sample-kind",
                "timestamp": "2026-01-01T00:00:00Z",
                "text": "text",
            }))
            .await;
        let body: Value = res.json();
        let id = Uuid::parse_str(body["id"].as_str().expect("id")).expect("uuid");
        assert_eq!(
            (res.status_code(), normalize_annotation_response(body)),
            (
                StatusCode::CREATED,
                json!({
                    "id": "<id>",
                    "target_symbol": target_symbol,
                    "target_kind": "sample-kind",
                    "timestamp": "2026-01-01T00:00:00Z",
                    "price": null,
                    "text": "text",
                    "status": "unread",
                    "linked_note_id": null,
                    "created_by_kind": "human",
                    "created_at": "<created_at>",
                    "updated_at": "<updated_at>",
                    "execution_step_id": null,
                    "execution_task_id": null,
                }),
            ),
        );
        id
    }

    async fn create_annotation_at(
        server: &TestServer,
        target_symbol: &str,
        timestamp: &str,
    ) -> Value {
        let response = server
            .post("/api/annotations")
            .json(&json!({
                "target_symbol": target_symbol,
                "target_kind": "sample-kind",
                "timestamp": timestamp,
                "text": "sample text",
            }))
            .await;
        let body = response.json::<Value>();
        assert_eq!(
            (
                response.status_code(),
                normalize_annotation_response(body.clone())
            ),
            (
                StatusCode::CREATED,
                json!({
                    "id": "<id>",
                    "target_symbol": target_symbol,
                    "target_kind": "sample-kind",
                    "timestamp": timestamp,
                    "price": null,
                    "text": "sample text",
                    "status": "unread",
                    "linked_note_id": null,
                    "created_by_kind": "human",
                    "created_at": "<created_at>",
                    "updated_at": "<updated_at>",
                    "execution_step_id": null,
                    "execution_task_id": null,
                }),
            ),
        );
        body
    }

    fn expected_annotation_response(timestamp: &str) -> Value {
        json!({
            "id": "<id>",
            "target_symbol": "SAMPLE-A",
            "target_kind": "sample-kind",
            "timestamp": timestamp,
            "price": null,
            "text": "sample text",
            "status": "unread",
            "linked_note_id": null,
            "created_by_kind": "human",
            "created_at": "<created_at>",
            "updated_at": "<updated_at>",
            "execution_step_id": null,
            "execution_task_id": null,
        })
    }

    #[backend_test_macros::database_test]
    async fn list_annotations_filters_by_symbol_newest_first(db: gateway_postgres::DatabaseHandle) {
        let (_db, server) = create_test_server_with_db(db).await;
        create_annotation_at(&server, "SAMPLE-A", "2026-06-01T00:00:00Z").await;
        create_annotation_at(&server, "SAMPLE-B", "2026-06-03T00:00:00Z").await;
        create_annotation_at(&server, "SAMPLE-A", "2026-06-02T00:00:00Z").await;
        create_annotation_at(&server, "SAMPLE-A", "2026-06-04T00:00:00Z").await;

        let list = server.get("/api/annotations?target_symbol=SAMPLE-A").await;
        assert_eq!(
            (
                list.status_code(),
                normalize_annotation_response(list.json())
            ),
            (
                StatusCode::OK,
                json!([
                    expected_annotation_response("2026-06-04T00:00:00Z"),
                    expected_annotation_response("2026-06-02T00:00:00Z"),
                    expected_annotation_response("2026-06-01T00:00:00Z"),
                ]),
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn get_annotation_returns_annotation(db: gateway_postgres::DatabaseHandle) {
        let (_db, server) = create_test_server_with_db(db).await;
        let created = create_annotation_at(&server, "SAMPLE-A", "2026-06-01T00:00:00Z").await;
        let response = server
            .get(&format!(
                "/api/annotations/{}",
                created["id"].as_str().expect("id")
            ))
            .await;

        assert_eq!(
            (
                response.status_code(),
                normalize_annotation_response(response.json())
            ),
            (
                StatusCode::OK,
                expected_annotation_response("2026-06-01T00:00:00Z"),
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn get_annotation_returns_not_found_for_missing_id(db: gateway_postgres::DatabaseHandle) {
        let (_db, server) = create_test_server_with_db(db).await;
        let missing_id = Uuid::nil();
        let response = server.get(&format!("/api/annotations/{missing_id}")).await;

        assert_eq!(
            (response.status_code(), response.json::<Value>()),
            (
                StatusCode::NOT_FOUND,
                json!({ "error": format!("annotation {missing_id} not found") }),
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn create_annotation_succeeds_without_strategy_id(db: gateway_postgres::DatabaseHandle) {
        let (_db, server) = create_test_server_with_db(db).await;

        let res = server
            .post("/api/annotations")
            .json(&json!({
                "target_symbol": "N225",
                "target_kind": "sample-kind",
                "timestamp": "2026-01-01T00:00:00Z",
                "text": "市況アノテーション",
            }))
            .await;
        let mut body: Value = res.json();
        let obj = body.as_object_mut().unwrap();
        for key in ["id", "created_at", "updated_at"] {
            obj.insert(key.into(), json!(format!("<{key}>")));
        }
        assert_eq!(
            (res.status_code(), body),
            (
                StatusCode::CREATED,
                json!({
                    "id": "<id>",
                    "target_symbol": "N225",
                    "target_kind": "sample-kind",
                    "timestamp": "2026-01-01T00:00:00Z",
                    "price": null,
                    "text": "市況アノテーション",
                    "status": "unread",
                    "linked_note_id": null,
                    "created_by_kind": "human",
                    "created_at": "<created_at>",
                    "updated_at": "<updated_at>",
                    "execution_step_id": null,
                    "execution_task_id": null,
                }),
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn create_annotation_preserves_a_numeric_price(db: gateway_postgres::DatabaseHandle) {
        let (_db, server) = create_test_server_with_db(db).await;
        let response = server
            .post("/api/annotations")
            .json(&json!({
                "target_symbol": "FICTIONAL-ASSET",
                "target_kind": "sample-tag",
                "timestamp": "2030-01-02T00:00:00Z",
                "price": 17.25,
                "text": "sample annotation",
            }))
            .await;

        assert_eq!(
            (
                response.status_code(),
                normalize_annotation_response(response.json::<Value>()),
            ),
            (
                StatusCode::CREATED,
                json!({
                    "id": "<id>",
                    "target_symbol": "FICTIONAL-ASSET",
                    "target_kind": "sample-tag",
                    "timestamp": "2030-01-02T00:00:00Z",
                    "price": 17.25,
                    "text": "sample annotation",
                    "status": "unread",
                    "linked_note_id": null,
                    "created_by_kind": "human",
                    "created_at": "<created_at>",
                    "updated_at": "<updated_at>",
                    "execution_step_id": null,
                    "execution_task_id": null,
                }),
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn create_and_update_accept_linked_notes(db: gateway_postgres::DatabaseHandle) {
        let (db, server) = create_test_server_with_db(db).await;
        let created_note_id = insert_test_note(&db, "created note", "body").await;
        let updated_note_id = insert_test_note(&db, "updated note", "body").await;
        let initial_annotation_res = server
            .post("/api/annotations")
            .json(&json!({
                "target_symbol": "TEST-SYMBOL",
                "target_kind": "sample_kind",
                "timestamp": "2026-01-01T00:00:00Z",
                "text": "text",
            }))
            .await;
        let initial_annotation_body = initial_annotation_res.json::<Value>();
        let annotation_id =
            Uuid::parse_str(initial_annotation_body["id"].as_str().expect("id")).expect("uuid");
        assert_eq!(
            (
                initial_annotation_res.status_code(),
                normalize_annotation_response(initial_annotation_body),
            ),
            (
                StatusCode::CREATED,
                json!({
                    "id": "<id>",
                    "target_symbol": "TEST-SYMBOL",
                    "target_kind": "sample_kind",
                    "timestamp": "2026-01-01T00:00:00Z",
                    "price": null,
                    "text": "text",
                    "status": "unread",
                    "linked_note_id": null,
                    "created_by_kind": "human",
                    "created_at": "<created_at>",
                    "updated_at": "<updated_at>",
                    "execution_step_id": null,
                    "execution_task_id": null,
                }),
            ),
        );

        let create_res = server
            .post("/api/annotations")
            .json(&json!({
                "target_symbol": "TEST-SYMBOL",
                "target_kind": "sample_kind",
                "timestamp": "2026-01-01T00:00:00Z",
                "text": "text",
                "linked_note_id": created_note_id,
            }))
            .await;
        let created_annotation_body = create_res.json::<Value>();
        let created_annotation_id =
            Uuid::parse_str(created_annotation_body["id"].as_str().expect("id")).expect("uuid");
        let create_result = (
            create_res.status_code(),
            normalize_annotation_response(created_annotation_body),
        );

        let update_res = server
            .patch(&format!("/api/annotations/{annotation_id}"))
            .json(&json!({ "linked_note_id": updated_note_id }))
            .await;
        let update_result = (
            update_res.status_code(),
            normalize_annotation_response(update_res.json::<Value>()),
        );

        let mut saved_annotation_links = annotation::Entity::find()
            .all(&db)
            .await
            .unwrap()
            .into_iter()
            .map(|saved| (saved.id, saved.linked_note_id))
            .collect::<Vec<_>>();
        saved_annotation_links.sort_unstable_by_key(|(id, _)| *id);
        let mut expected_annotation_links = vec![
            (annotation_id, Some(updated_note_id)),
            (created_annotation_id, Some(created_note_id)),
        ];
        expected_annotation_links.sort_unstable_by_key(|(id, _)| *id);

        assert_eq!(
            (create_result, update_result, saved_annotation_links),
            (
                (
                    StatusCode::CREATED,
                    json!({
                        "id": "<id>",
                        "target_symbol": "TEST-SYMBOL",
                        "target_kind": "sample_kind",
                        "timestamp": "2026-01-01T00:00:00Z",
                        "price": null,
                        "text": "text",
                        "status": "unread",
                        "linked_note_id": created_note_id,
                        "created_by_kind": "human",
                        "created_at": "<created_at>",
                        "updated_at": "<updated_at>",
                        "execution_step_id": null,
                        "execution_task_id": null,
                    }),
                ),
                (
                    StatusCode::OK,
                    json!({
                        "id": "<id>",
                        "target_symbol": "TEST-SYMBOL",
                        "target_kind": "sample_kind",
                        "timestamp": "2026-01-01T00:00:00Z",
                        "price": null,
                        "text": "text",
                        "status": "unread",
                        "linked_note_id": updated_note_id,
                        "created_by_kind": "human",
                        "created_at": "<created_at>",
                        "updated_at": "<updated_at>",
                        "execution_step_id": null,
                        "execution_task_id": null,
                    }),
                ),
                expected_annotation_links,
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn reject_annotation_without_execution_step_does_not_submit_task(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let fake = Arc::new(FakeAgentTaskClient::new());
        let agent_client: SharedAgentTaskClient = fake.clone();
        let (db, server) = create_test_server_with_db_and_agent_client(db, agent_client).await;
        let anno_id = create_test_annotation_with_symbol(&server, "sample-symbol").await;

        let res = server
            .post(&format!("/api/annotations/{anno_id}/reject"))
            .json(&json!({}))
            .await;
        assert_eq!(
            (
                res.status_code(),
                normalize_annotation_response(res.json()),
                strategy_task::Entity::find().all(&db).await.unwrap(),
            ),
            (
                StatusCode::OK,
                json!({
                    "id": "<id>",
                    "target_symbol": "sample-symbol",
                    "target_kind": "sample-kind",
                    "timestamp": "2026-01-01T00:00:00Z",
                    "price": null,
                    "text": "text",
                    "status": "rejected",
                    "linked_note_id": null,
                    "created_by_kind": "human",
                    "created_at": "<created_at>",
                    "updated_at": "<updated_at>",
                    "execution_step_id": null,
                    "execution_task_id": null,
                }),
                vec![],
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn reject_annotation_with_unrecorded_execution_step_does_not_submit_task(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let fake = Arc::new(FakeAgentTaskClient::new());
        let agent_client: SharedAgentTaskClient = fake.clone();
        let (db, server) = create_test_server_with_db_and_agent_client(db, agent_client).await;
        let anno_id = create_test_annotation_with_symbol(&server, "sample-symbol").await;
        let execution_step_id = Uuid::from_u128(501);
        set_test_annotation_execution_step_id(&db, anno_id, execution_step_id).await;

        let res = server
            .post(&format!("/api/annotations/{anno_id}/reject"))
            .json(&json!({}))
            .await;
        assert_eq!(
            (
                res.status_code(),
                normalize_annotation_response(res.json()),
                strategy_task::Entity::find().all(&db).await.unwrap(),
            ),
            (
                StatusCode::OK,
                json!({
                    "id": "<id>",
                    "target_symbol": "sample-symbol",
                    "target_kind": "sample-kind",
                    "timestamp": "2026-01-01T00:00:00Z",
                    "price": null,
                    "text": "text",
                    "status": "rejected",
                    "linked_note_id": null,
                    "created_by_kind": "human",
                    "created_at": "<created_at>",
                    "updated_at": "<updated_at>",
                    "execution_step_id": execution_step_id,
                    "execution_task_id": null,
                }),
                vec![],
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn reject_annotation_uses_execution_strategy_for_review_task(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let fake = Arc::new(FakeAgentTaskClient::new());
        let agent_client: SharedAgentTaskClient = fake.clone();
        let (db, server) = create_test_server_with_db_and_agent_client(db, agent_client).await;
        let task_strategy_id = insert_test_strategy(&db, "sample-task-strategy").await;
        agent_config::create(&db, DEFAULT_PURPOSE.to_string())
            .await
            .expect("insert test agent_config");
        let anno_id = create_test_annotation_with_symbol(&server, "sample-symbol").await;
        let execution_step_id = Uuid::from_u128(502);
        insert_test_strategy_task_step(&db, task_strategy_id, execution_step_id).await;
        set_test_annotation_execution_step_id(&db, anno_id, execution_step_id).await;

        let res = server
            .post(&format!("/api/annotations/{anno_id}/reject"))
            .json(&json!({}))
            .await;
        let tasks = strategy_task::Entity::find()
            .filter(strategy_task::Column::Source.eq("review"))
            .all(&db)
            .await
            .unwrap();
        assert_eq!(
            (
                res.status_code(),
                normalize_annotation_response(res.json()),
                tasks.iter().map(TaskShape::from).collect::<Vec<_>>(),
            ),
            (
                StatusCode::OK,
                json!({
                "id": "<id>",
                "target_symbol": "sample-symbol",
                "target_kind": "sample-kind",
                "timestamp": "2026-01-01T00:00:00Z",
                "price": null,
                "text": "text",
                "status": "rejected",
                "linked_note_id": null,
                "created_by_kind": "human",
                "created_at": "<created_at>",
                "updated_at": "<updated_at>",
                "execution_step_id": execution_step_id,
                "execution_task_id": null,
                }),
                vec![TaskShape {
                    strategy_id: task_strategy_id,
                    source: "review".to_string(),
                    prompt: format!(
                        "アノテーション (id: {anno_id}, 対象: sample-symbol) がレビューで却下されました。付いているコメントを確認し、指摘を反映してください。"
                    ),
                    phase: StrategyTaskPhase::Running,
                }],
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn rejecting_already_rejected_annotation_does_not_resubmit(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let fake = Arc::new(FakeAgentTaskClient::new());
        let agent_client: SharedAgentTaskClient = fake.clone();
        let (db, server) = create_test_server_with_db_and_agent_client(db, agent_client).await;
        let strategy_id = insert_test_strategy(&db, "s").await;
        agent_config::create(&db, DEFAULT_PURPOSE.to_string())
            .await
            .expect("insert test agent_config");
        let anno_id = create_test_annotation(&server).await;
        let execution_step_id = Uuid::from_u128(603);
        insert_test_strategy_task_step(&db, strategy_id, execution_step_id).await;
        set_test_annotation_execution_step_id(&db, anno_id, execution_step_id).await;

        for _ in 0..2 {
            let res = server
                .post(&format!("/api/annotations/{anno_id}/reject"))
                .json(&json!({}))
                .await;
            assert_eq!(
                (res.status_code(), normalize_annotation_response(res.json())),
                (
                    StatusCode::OK,
                    json!({
                        "id": "<id>",
                        "target_symbol": "sample-code",
                        "target_kind": "sample-kind",
                        "timestamp": "2026-01-01T00:00:00Z",
                        "price": null,
                        "text": "text",
                        "status": "rejected",
                        "linked_note_id": null,
                        "created_by_kind": "human",
                        "created_at": "<created_at>",
                        "updated_at": "<updated_at>",
                        "execution_step_id": execution_step_id,
                        "execution_task_id": null,
                    }),
                ),
            );
        }

        let tasks = strategy_task::Entity::find()
            .filter(strategy_task::Column::Source.eq("review"))
            .filter(strategy_task::Column::StrategyId.eq(strategy_id))
            .all(&db)
            .await
            .unwrap();
        assert_eq!(tasks.len(), 1);
    }

    #[backend_test_macros::database_test]
    async fn reject_annotation_leaves_status_unchanged_when_agent_submission_fails(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let fake = Arc::new(FakeAgentTaskClient::new());
        fake.set_submit_error(AgentTaskError::NotConfigured).await;
        let agent_client: SharedAgentTaskClient = fake;
        let (db, server) = create_test_server_with_db_and_agent_client(db, agent_client).await;
        let strategy_id = insert_test_strategy(&db, "s").await;
        agent_config::create(&db, DEFAULT_PURPOSE.to_string())
            .await
            .expect("insert test agent_config");
        let anno_id = create_test_annotation(&server).await;
        let execution_step_id = Uuid::from_u128(604);
        insert_test_strategy_task_step(&db, strategy_id, execution_step_id).await;
        set_test_annotation_execution_step_id(&db, anno_id, execution_step_id).await;

        let res = server
            .post(&format!("/api/annotations/{anno_id}/reject"))
            .json(&json!({}))
            .await;
        assert_response_eq(
            &res,
            StatusCode::SERVICE_UNAVAILABLE,
            Some(json!({ "error": "agent task client is not configured" })),
        );

        let anno = annotation::Entity::find_by_id(anno_id)
            .one(&db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(anno.status, "unread");
    }
}
