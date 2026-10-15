#[cfg(test)]
mod tests {
    use super::super::assert_response_eq;
    use chrono::Utc;
    use sea_orm::ActiveModelTrait;
    use sea_orm::ActiveValue::{NotSet, Set};
    use sea_orm::ConnectionTrait;
    use serde_json::json;
    use uuid::Uuid;

    use crate::testing::{
        create_test_server_with_db, insert_test_strategy, insert_test_strategy_task_step,
    };
    use gateway_postgres::entities::sea_orm_active_enums::StrategyTaskStepStatus;
    use gateway_postgres::entities::strategy_task_step;

    #[backend_test_macros::database_test]
    async fn get_strategy_task_step_origin_returns_its_strategy_and_task(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let (db, server) = create_test_server_with_db(db).await;
        let strategy_id = insert_test_strategy(&db, "sample-strategy").await;
        let execution_step_id = Uuid::new_v4();
        let task_id = insert_test_strategy_task_step(&db, strategy_id, execution_step_id).await;

        let response = server
            .get(&format!("/api/strategy-task-steps/{execution_step_id}"))
            .await;

        assert_response_eq(
            &response,
            axum::http::StatusCode::OK,
            Some(json!({
                "strategy_id": strategy_id,
                "task_id": task_id,
            })),
        );
    }

    #[backend_test_macros::database_test]
    async fn get_strategy_task_step_origin_returns_not_found_when_step_is_missing(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let (_db, server) = create_test_server_with_db(db).await;

        let missing_step_id = Uuid::new_v4();
        let response = server
            .get(&format!("/api/strategy-task-steps/{missing_step_id}"))
            .await;
        assert_response_eq(
            &response,
            axum::http::StatusCode::NOT_FOUND,
            Some(json!({
                "error": format!(
                    "strategy task origin for execution step {missing_step_id} not found"
                ),
            })),
        );
    }

    #[backend_test_macros::database_test]
    async fn get_strategy_task_step_origin_returns_not_found_when_parent_task_is_missing(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let (db, server) = create_test_server_with_db(db).await;
        let execution_step_id = Uuid::new_v4();
        let missing_task_id = Uuid::new_v4();
        let now = Utc::now().fixed_offset();
        db.execute_unprepared("ALTER TABLE strategy_task_step DISABLE TRIGGER ALL")
            .await
            .expect("disable step foreign key trigger");
        strategy_task_step::ActiveModel {
            execution_step_id: Set(execution_step_id),
            task_id: Set(missing_task_id),
            phase_key: Set("sample-phase".to_string()),
            label: Set("Sample step".to_string()),
            model: Set("fictional-model".to_string()),
            status: Set(StrategyTaskStepStatus::Completed),
            item: Set(None),
            item_label: Set(None),
            output: Set(None),
            started_at: Set(now),
            finished_at: Set(Some(now)),
            trace_id: Set("sample-trace".to_string()),
            span_id: Set("sample-span".to_string()),
            error: Set(None),
            seq: NotSet,
        }
        .insert(&db)
        .await
        .expect("insert step without parent task");
        db.execute_unprepared("ALTER TABLE strategy_task_step ENABLE TRIGGER ALL")
            .await
            .expect("enable step foreign key trigger");

        let response = server
            .get(&format!("/api/strategy-task-steps/{execution_step_id}"))
            .await;
        assert_response_eq(
            &response,
            axum::http::StatusCode::NOT_FOUND,
            Some(json!({
                "error": format!(
                    "strategy task origin for execution step {execution_step_id} not found"
                ),
            })),
        );
    }
}
