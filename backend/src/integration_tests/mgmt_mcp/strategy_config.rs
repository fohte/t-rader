use super::dto::*;

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use rmcp::handler::server::wrapper::{Json, Parameters};
    use serde_json::json;
    use uuid::Uuid;

    use crate::agent_client::FakeAgentTaskClient;
    use crate::testing::insert_test_cron_trigger;

    use super::super::tests_common::{build_server, insert_strategy};
    use super::*;

    #[backend_test_macros::database_test]
    async fn get_strategy_config_returns_full_row_and_empty_triggers(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let strategy_id = insert_strategy(&db, "s").await;
        let server = build_server(db, Arc::new(FakeAgentTaskClient::new()));

        let Json(result) = server
            .get_strategy_config(Parameters(GetStrategyConfigParams { strategy_id }))
            .await
            .expect("ok");

        assert_eq!(
            serde_json::to_value(result).unwrap(),
            json!({
                "strategy_id": strategy_id,
                "name": "s",
                "description": null,
                "triggers": [],
            }),
        );
    }

    #[backend_test_macros::database_test]
    async fn get_strategy_config_includes_triggers(db: gateway_postgres::DatabaseHandle) {
        let strategy_id = insert_strategy(&db, "s").await;
        let trigger_id =
            insert_test_cron_trigger(&db, strategy_id, "0 9 * * *", true, None, "prompt", None)
                .await;
        let server = build_server(db, Arc::new(FakeAgentTaskClient::new()));

        let Json(result) = server
            .get_strategy_config(Parameters(GetStrategyConfigParams { strategy_id }))
            .await
            .expect("ok");

        assert_eq!(
            (
                result.triggers.len(),
                result.triggers.first().map(|t| t.trigger_id)
            ),
            (1, Some(trigger_id)),
        );
    }

    #[backend_test_macros::database_test]
    async fn get_strategy_config_rejects_unknown_strategy(db: gateway_postgres::DatabaseHandle) {
        let server = build_server(db, Arc::new(FakeAgentTaskClient::new()));

        let err = server
            .get_strategy_config(Parameters(GetStrategyConfigParams {
                strategy_id: Uuid::new_v4(),
            }))
            .await
            .err()
            .unwrap_or_else(|| panic!("expected error"));
        assert_eq!(err.code, rmcp::model::ErrorCode::INVALID_PARAMS);
    }
}
