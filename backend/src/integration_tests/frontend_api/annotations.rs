#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use crate::testing::agent_config;
    use crate::testing::{
        create_test_server_with_db, create_test_server_with_db_and_agent_client, insert_test_note,
        insert_test_strategy,
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

    /// strategy_task 行の動的フィールド (id / 時刻 / a2a_task_id) を捨てた比較用ビュー。
    #[derive(Debug, PartialEq, Eq)]
    struct TaskShape {
        strategy_id: Uuid,
        source: String,
        prompt: String,
        phase: StrategyTaskPhase,
    }

    impl TaskShape {
        fn from(row: &strategy_task::Model) -> Self {
            Self {
                strategy_id: row.strategy_id,
                source: row.source.clone(),
                prompt: row.prompt.clone(),
                phase: row.phase.clone(),
            }
        }
    }

    async fn create_test_annotation(server: &TestServer, strategy_id: Uuid) -> Uuid {
        let res = server
            .post("/api/annotations")
            .json(&json!({
                "strategy_id": strategy_id,
                "target_symbol": "7203",
                "target_kind": "observation",
                "timestamp": "2026-01-01T00:00:00Z",
                "text": "text",
            }))
            .await;
        res.assert_status(StatusCode::CREATED);
        let body: Value = res.json();
        Uuid::parse_str(body["id"].as_str().expect("id")).expect("uuid")
    }

    async fn create_annotation_at(
        server: &TestServer,
        strategy_id: Uuid,
        target_symbol: &str,
        timestamp: &str,
    ) -> Value {
        let response = server
            .post("/api/annotations")
            .json(&json!({
                "strategy_id": strategy_id,
                "target_symbol": target_symbol,
                "target_kind": "sample-kind",
                "timestamp": timestamp,
                "text": "sample text",
            }))
            .await;
        response.assert_status(StatusCode::CREATED);
        response.json()
    }

    fn normalize_annotation_response(mut value: Value) -> Value {
        fn normalize_row(annotation: &mut Value) {
            for key in ["id", "created_at", "updated_at"] {
                if let Some(value) = annotation
                    .as_object_mut()
                    .and_then(|object| object.get_mut(key))
                {
                    *value = Value::String(format!("<{key}>"));
                }
            }
        }
        if let Some(rows) = value.as_array_mut() {
            for row in rows {
                normalize_row(row);
            }
        } else {
            normalize_row(&mut value);
        }
        value
    }

    async fn create_annotation_read_context(
        db: gateway_postgres::DatabaseHandle,
    ) -> (gateway_postgres::DatabaseHandle, TestServer, Uuid) {
        let (db, server) = create_test_server_with_db(db).await;
        let strategy_id = insert_test_strategy(&db, "sample-strategy").await;
        (db, server, strategy_id)
    }

    fn expected_annotation_response(strategy_id: Uuid, timestamp: &str) -> Value {
        json!({
            "id": "<id>",
            "strategy_id": strategy_id,
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
    async fn list_annotations_filters_by_strategy_and_symbol_newest_first(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let (db, server, strategy_id) = create_annotation_read_context(db).await;
        let foreign_strategy_id = insert_test_strategy(&db, "foreign-strategy").await;
        create_annotation_at(&server, strategy_id, "SAMPLE-A", "2026-06-01T00:00:00Z").await;
        create_annotation_at(&server, strategy_id, "SAMPLE-B", "2026-06-03T00:00:00Z").await;
        create_annotation_at(&server, strategy_id, "SAMPLE-A", "2026-06-02T00:00:00Z").await;
        create_annotation_at(
            &server,
            foreign_strategy_id,
            "SAMPLE-A",
            "2026-06-04T00:00:00Z",
        )
        .await;

        let list = server
            .get(&format!(
                "/api/annotations?strategy_id={strategy_id}&target_symbol=SAMPLE-A"
            ))
            .await;
        assert_eq!(
            (
                list.status_code(),
                normalize_annotation_response(list.json())
            ),
            (
                StatusCode::OK,
                json!([
                    expected_annotation_response(strategy_id, "2026-06-02T00:00:00Z"),
                    expected_annotation_response(strategy_id, "2026-06-01T00:00:00Z"),
                ]),
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn get_annotation_returns_annotation(db: gateway_postgres::DatabaseHandle) {
        let (_db, server, strategy_id) = create_annotation_read_context(db).await;
        let created =
            create_annotation_at(&server, strategy_id, "SAMPLE-A", "2026-06-01T00:00:00Z").await;
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
                expected_annotation_response(strategy_id, "2026-06-01T00:00:00Z"),
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn get_annotation_returns_not_found_for_missing_id(db: gateway_postgres::DatabaseHandle) {
        let (_db, server, _strategy_id) = create_annotation_read_context(db).await;
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
    async fn create_annotation_without_strategy_id_succeeds(db: gateway_postgres::DatabaseHandle) {
        let (_db, server) = create_test_server_with_db(db).await;

        let res = server
            .post("/api/annotations")
            .json(&json!({
                "target_symbol": "N225",
                "target_kind": "observation",
                "timestamp": "2026-01-01T00:00:00Z",
                "text": "市況アノテーション",
            }))
            .await;
        res.assert_status(StatusCode::CREATED);
        let mut body: Value = res.json();
        let obj = body.as_object_mut().unwrap();
        obj.remove("id");
        obj.remove("created_at");
        obj.remove("updated_at");
        assert_eq!(
            body,
            json!({
                "strategy_id": null,
                "target_symbol": "N225",
                "target_kind": "observation",
                "timestamp": "2026-01-01T00:00:00Z",
                "price": null,
                "text": "市況アノテーション",
                "status": "unread",
                "linked_note_id": null,
                "created_by_kind": "human",
                "execution_step_id": null,
                "execution_task_id": null,
            }),
        );
    }

    #[backend_test_macros::database_test]
    async fn create_and_update_reject_cross_strategy_linked_notes(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let (db, server) = create_test_server_with_db(db).await;
        let strategy_id = insert_test_strategy(&db, "owner").await;
        let foreign_strategy_id = insert_test_strategy(&db, "foreign").await;
        let foreign_note_id =
            insert_test_note(&db, foreign_strategy_id, "foreign note", "body").await;
        let initial_annotation_res = server
            .post("/api/annotations")
            .json(&json!({
                "strategy_id": strategy_id,
                "target_symbol": "TEST-SYMBOL",
                "target_kind": "sample_kind",
                "timestamp": "2026-01-01T00:00:00Z",
                "text": "text",
            }))
            .await;
        initial_annotation_res.assert_status(StatusCode::CREATED);
        let annotation_id = Uuid::parse_str(
            initial_annotation_res.json::<Value>()["id"]
                .as_str()
                .expect("id"),
        )
        .expect("uuid");

        let create_res = server
            .post("/api/annotations")
            .json(&json!({
                "strategy_id": strategy_id,
                "target_symbol": "TEST-SYMBOL",
                "target_kind": "sample_kind",
                "timestamp": "2026-01-01T00:00:00Z",
                "text": "text",
                "linked_note_id": foreign_note_id,
            }))
            .await;
        let create_result = (create_res.status_code(), create_res.json::<Value>());

        let update_res = server
            .patch(&format!("/api/annotations/{annotation_id}"))
            .json(&json!({ "linked_note_id": foreign_note_id }))
            .await;
        let update_result = (update_res.status_code(), update_res.json::<Value>());

        let saved_annotations = annotation::Entity::find()
            .all(&db)
            .await
            .unwrap()
            .into_iter()
            .map(|saved| (saved.id, saved.linked_note_id))
            .collect::<Vec<_>>();

        assert_eq!(
            (create_result, update_result, saved_annotations),
            (
                (
                    StatusCode::BAD_REQUEST,
                    json!({ "error": "linked note belongs to a different strategy" }),
                ),
                (
                    StatusCode::BAD_REQUEST,
                    json!({ "error": "linked note belongs to a different strategy" }),
                ),
                vec![(annotation_id, None)],
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn reject_annotation_without_strategy_id_does_not_submit_task(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let fake = Arc::new(FakeAgentTaskClient::new());
        let agent_client: SharedAgentTaskClient = fake.clone();
        let (db, server) = create_test_server_with_db_and_agent_client(db, agent_client).await;

        let res = server
            .post("/api/annotations")
            .json(&json!({
                "target_symbol": "N225",
                "target_kind": "observation",
                "timestamp": "2026-01-01T00:00:00Z",
                "text": "市況アノテーション",
            }))
            .await;
        res.assert_status(StatusCode::CREATED);
        let anno_id =
            Uuid::parse_str(res.json::<Value>()["id"].as_str().expect("id")).expect("uuid");

        let res = server
            .post(&format!("/api/annotations/{anno_id}/reject"))
            .json(&json!({}))
            .await;
        res.assert_status_ok();
        let mut body: Value = res.json();
        let obj = body.as_object_mut().unwrap();
        obj.remove("id");
        obj.remove("created_at");
        obj.remove("updated_at");
        assert_eq!(
            body,
            json!({
                "strategy_id": null,
                "target_symbol": "N225",
                "target_kind": "observation",
                "timestamp": "2026-01-01T00:00:00Z",
                "price": null,
                "text": "市況アノテーション",
                "status": "rejected",
                "linked_note_id": null,
                "created_by_kind": "human",
                "execution_step_id": null,
                "execution_task_id": null,
            }),
        );

        let tasks = strategy_task::Entity::find().all(&db).await.unwrap();
        assert_eq!(tasks, vec![]);
    }

    #[backend_test_macros::database_test]
    async fn reject_annotation_submits_single_review_task_referencing_annotation(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let fake = Arc::new(FakeAgentTaskClient::new());
        let agent_client: SharedAgentTaskClient = fake.clone();
        let (db, server) = create_test_server_with_db_and_agent_client(db, agent_client).await;
        let strategy_id = insert_test_strategy(&db, "s").await;
        agent_config::create(&db, DEFAULT_PURPOSE.to_string())
            .await
            .expect("insert test agent_config");
        let anno_id = create_test_annotation(&server, strategy_id).await;

        let res = server
            .post(&format!("/api/annotations/{anno_id}/reject"))
            .json(&json!({}))
            .await;
        res.assert_status_ok();
        let mut body: Value = res.json();
        let obj = body.as_object_mut().unwrap();
        obj.remove("created_at");
        obj.remove("updated_at");
        assert_eq!(
            body,
            json!({
                "id": anno_id,
                "strategy_id": strategy_id,
                "target_symbol": "7203",
                "target_kind": "observation",
                "timestamp": "2026-01-01T00:00:00Z",
                "price": null,
                "text": "text",
                "status": "rejected",
                "linked_note_id": null,
                "created_by_kind": "human",
                "execution_step_id": null,
                "execution_task_id": null,
            }),
        );

        let tasks = strategy_task::Entity::find()
            .filter(strategy_task::Column::StrategyId.eq(strategy_id))
            .all(&db)
            .await
            .unwrap();
        assert_eq!(
            tasks.iter().map(TaskShape::from).collect::<Vec<_>>(),
            vec![TaskShape {
                strategy_id,
                source: "review".to_string(),
                prompt: format!(
                    "アノテーション (id: {anno_id}, 対象: 7203) がレビューで却下されました。\
付いているコメントを確認し、指摘を反映してください。"
                ),
                phase: StrategyTaskPhase::Running,
            }],
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
        let anno_id = create_test_annotation(&server, strategy_id).await;

        for _ in 0..2 {
            let res = server
                .post(&format!("/api/annotations/{anno_id}/reject"))
                .json(&json!({}))
                .await;
            res.assert_status_ok();
        }

        let tasks = strategy_task::Entity::find()
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
        let anno_id = create_test_annotation(&server, strategy_id).await;

        let res = server
            .post(&format!("/api/annotations/{anno_id}/reject"))
            .json(&json!({}))
            .await;
        res.assert_status(StatusCode::SERVICE_UNAVAILABLE);

        let anno = annotation::Entity::find_by_id(anno_id)
            .one(&db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(anno.status, "unread");
    }
}
