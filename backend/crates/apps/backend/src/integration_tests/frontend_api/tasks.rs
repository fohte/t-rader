#[cfg(test)]
mod tests {
    use super::super::{assert_response_eq, normalize_timestamps};
    use crate::testing::{
        create_test_server, create_test_server_with_db, insert_test_strategy,
        insert_test_strategy_task, insert_test_strategy_task_step_for_task,
    };
    use gateway_postgres::entities::strategy_task_step;
    use sea_orm::ActiveModelTrait;
    use sea_orm::ActiveValue::Set;
    use serde_json::json;
    use uuid::Uuid;

    #[backend_test_macros::database_test]
    async fn list_tasks_returns_all_strategies_newest_first(db: gateway_postgres::DatabaseHandle) {
        let (db, server) = create_test_server_with_db(db).await;
        let strategy_a = insert_test_strategy(&db, "a").await;
        let strategy_b = insert_test_strategy(&db, "b").await;

        let base = chrono::Utc::now().fixed_offset();
        let task_a = insert_test_strategy_task(&db, strategy_a, "for-a", None, base).await;
        let task_b = insert_test_strategy_task(
            &db,
            strategy_b,
            "for-b",
            None,
            base + chrono::Duration::seconds(1),
        )
        .await;

        let res = server.get("/api/tasks").await;
        let mut body: Vec<serde_json::Value> = res.json();
        body.iter_mut().for_each(normalize_timestamps);
        assert_eq!(
            (res.status_code(), body),
            (
                axum::http::StatusCode::OK,
                vec![
                    json!({
                        "task_id": task_b,
                        "strategy_id": strategy_b,
                        "source": "frontend",
                        "prompt": "for-b",
                        "phase": "completed",
                        "error_summary": null,
                        "model_call_limit_reached": false,
                        "created_at": "<created_at>",
                        "updated_at": "<updated_at>",
                        "purpose": null,
                        "as_of": "<as_of>",
                    }),
                    json!({
                        "task_id": task_a,
                        "strategy_id": strategy_a,
                        "source": "frontend",
                        "prompt": "for-a",
                        "phase": "completed",
                        "error_summary": null,
                        "model_call_limit_reached": false,
                        "created_at": "<created_at>",
                        "updated_at": "<updated_at>",
                        "purpose": null,
                        "as_of": "<as_of>",
                    }),
                ],
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn list_tasks_marks_task_when_a_step_reached_model_call_limit(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let (db, server) = create_test_server_with_db(db).await;
        let strategy_id = insert_test_strategy(&db, "x").await;
        let created_at = chrono::Utc::now().fixed_offset();
        let task_id =
            insert_test_strategy_task(&db, strategy_id, "for-limit", None, created_at).await;
        let execution_step_id = Uuid::from_u128(5);
        insert_test_strategy_task_step_for_task(&db, task_id, execution_step_id).await;
        strategy_task_step::ActiveModel {
            execution_step_id: Set(execution_step_id),
            model_call_limit_reached: Set(true),
            ..Default::default()
        }
        .update(&db)
        .await
        .expect("mark test strategy task step");

        let response = server.get("/api/tasks").await;
        let mut body: Vec<serde_json::Value> = response.json();
        body.iter_mut().for_each(normalize_timestamps);

        assert_eq!(
            (response.status_code(), body),
            (
                axum::http::StatusCode::OK,
                vec![json!({
                    "task_id": task_id,
                    "strategy_id": strategy_id,
                    "source": "frontend",
                    "prompt": "for-limit",
                    "phase": "completed",
                    "error_summary": null,
                    "model_call_limit_reached": true,
                    "created_at": "<created_at>",
                    "updated_at": "<updated_at>",
                    "purpose": null,
                    "as_of": "<as_of>",
                })],
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn list_tasks_filters_by_strategy_id(db: gateway_postgres::DatabaseHandle) {
        let (db, server) = create_test_server_with_db(db).await;
        let strategy_a = insert_test_strategy(&db, "a").await;
        let strategy_b = insert_test_strategy(&db, "b").await;

        let base = chrono::Utc::now().fixed_offset();
        let task_a = insert_test_strategy_task(&db, strategy_a, "for-a", None, base).await;
        insert_test_strategy_task(&db, strategy_b, "for-b", None, base).await;

        let res = server
            .get(&format!("/api/tasks?strategy_id={strategy_a}"))
            .await;
        let mut body: Vec<serde_json::Value> = res.json();
        body.iter_mut().for_each(normalize_timestamps);
        assert_eq!(
            (res.status_code(), body),
            (
                axum::http::StatusCode::OK,
                vec![json!({
                    "task_id": task_a,
                    "strategy_id": strategy_a,
                    "source": "frontend",
                    "prompt": "for-a",
                    "phase": "completed",
                    "error_summary": null,
                    "model_call_limit_reached": false,
                    "created_at": "<created_at>",
                    "updated_at": "<updated_at>",
                    "purpose": null,
                    "as_of": "<as_of>",
                })],
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn list_tasks_filters_by_purpose(db: gateway_postgres::DatabaseHandle) {
        let (db, server) = create_test_server_with_db(db).await;
        let strategy_id = insert_test_strategy(&db, "x").await;

        let base = chrono::Utc::now().fixed_offset();
        let task_inspect =
            insert_test_strategy_task(&db, strategy_id, "inspect prompt", Some("inspect"), base)
                .await;
        insert_test_strategy_task(
            &db,
            strategy_id,
            "default prompt",
            None,
            base + chrono::Duration::seconds(1),
        )
        .await;

        let res = server.get("/api/tasks?purpose=inspect").await;
        let mut body: Vec<serde_json::Value> = res.json();
        body.iter_mut().for_each(normalize_timestamps);
        assert_eq!(
            (res.status_code(), body),
            (
                axum::http::StatusCode::OK,
                vec![json!({
                    "task_id": task_inspect,
                    "strategy_id": strategy_id,
                    "source": "frontend",
                    "prompt": "inspect prompt",
                    "phase": "completed",
                    "error_summary": null,
                    "model_call_limit_reached": false,
                    "created_at": "<created_at>",
                    "updated_at": "<updated_at>",
                    "purpose": "inspect",
                    "as_of": "<as_of>",
                })],
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn list_tasks_returns_empty_for_unknown_strategy_id(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let server = create_test_server(db).await;

        let res = server
            .get("/api/tasks?strategy_id=00000000-0000-0000-0000-000000000000")
            .await;
        assert_response_eq(
            &res,
            axum::http::StatusCode::OK,
            Some(serde_json::json!([])),
        );
    }
}
