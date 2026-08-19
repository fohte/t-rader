//! 管理 MCP の戦略 Agent 多段フェーズ実行設定 (agent_graph YAML) tool。
//!
//! Web UI の PUT /api/strategies/{id}/agent-graph と同じ service を経由するため、
//! 検証・保存・履歴記録は `services::agent_graph` に一本化されている。

use rmcp::ErrorData as McpError;

use crate::error::AppError;
use crate::services::agent_graph as agent_graph_svc;

use super::dto::{
    GetStrategyAgentConfigParams, GetStrategyAgentConfigResult, PutStrategyAgentConfigParams,
    PutStrategyAgentConfigResult,
};
use super::{MgmtServer, db_error, internal_error, invalid_params};

impl MgmtServer {
    pub(super) async fn get_strategy_agent_config_inner(
        &self,
        params: GetStrategyAgentConfigParams,
    ) -> Result<GetStrategyAgentConfigResult, McpError> {
        let yaml = agent_graph_svc::get_agent_graph(&self.db, params.strategy_id)
            .await
            .map_err(map_app_error)?;
        Ok(GetStrategyAgentConfigResult { yaml })
    }

    pub(super) async fn put_strategy_agent_config_inner(
        &self,
        params: PutStrategyAgentConfigParams,
    ) -> Result<PutStrategyAgentConfigResult, McpError> {
        match agent_graph_svc::save_agent_graph(&self.db, params.strategy_id, &params.yaml).await {
            Ok(_) => Ok(PutStrategyAgentConfigResult {
                ok: true,
                errors: vec![],
            }),
            Err(AppError::Validation(msg)) => Ok(PutStrategyAgentConfigResult {
                ok: false,
                errors: vec![msg],
            }),
            Err(other) => Err(map_app_error(other)),
        }
    }
}

fn map_app_error(err: AppError) -> McpError {
    match err {
        AppError::NotFound(msg) => invalid_params(msg),
        AppError::Database(db_err) => db_error(db_err),
        other => internal_error(other.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use indoc::indoc;
    use rmcp::handler::server::wrapper::{Json, Parameters};
    use sqlx::PgPool;
    use uuid::Uuid;

    use crate::agent_client::FakeAgentTaskClient;
    use crate::testing::create_test_db;

    use super::super::tests_common::{build_server, insert_strategy};
    use super::*;

    #[sqlx::test(migrations = false)]
    async fn get_strategy_agent_config_defaults_to_empty_yaml(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_id = insert_strategy(&db, "s").await;
        let server = build_server(db, Arc::new(FakeAgentTaskClient::new()));

        let Json(result) = server
            .get_strategy_agent_config(Parameters(GetStrategyAgentConfigParams { strategy_id }))
            .await
            .expect("ok");
        assert_eq!(result.yaml, "");
    }

    #[sqlx::test(migrations = false)]
    async fn put_then_get_strategy_agent_config_round_trips(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_id = insert_strategy(&db, "s").await;
        let server = build_server(db, Arc::new(FakeAgentTaskClient::new()));

        let yaml = indoc! {"
            phases:
              - key: plan
                label: 調査計画
                model: claude-opus-4
                prompt: 仮説を立てよ
        "};

        let Json(put_result) = server
            .put_strategy_agent_config(Parameters(PutStrategyAgentConfigParams {
                strategy_id,
                yaml: yaml.to_string(),
            }))
            .await
            .expect("ok");
        assert_eq!(
            (put_result.ok, put_result.errors),
            (true, Vec::<String>::new()),
        );

        let Json(get_result) = server
            .get_strategy_agent_config(Parameters(GetStrategyAgentConfigParams { strategy_id }))
            .await
            .expect("ok");
        assert_eq!(get_result.yaml, yaml);
    }

    #[sqlx::test(migrations = false)]
    async fn put_strategy_agent_config_returns_errors_for_invalid_yaml(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_id = insert_strategy(&db, "s").await;
        let server = build_server(db, Arc::new(FakeAgentTaskClient::new()));

        let Json(result) = server
            .put_strategy_agent_config(Parameters(PutStrategyAgentConfigParams {
                strategy_id,
                yaml: "phases: [".to_string(),
            }))
            .await
            .expect("tool call itself must succeed");
        assert!(!result.ok);
        assert!(!result.errors.is_empty());
    }

    #[sqlx::test(migrations = false)]
    async fn put_strategy_agent_config_rejects_unknown_strategy(pool: PgPool) {
        let db = create_test_db(pool).await;
        let server = build_server(db, Arc::new(FakeAgentTaskClient::new()));

        let err = server
            .put_strategy_agent_config(Parameters(PutStrategyAgentConfigParams {
                strategy_id: Uuid::new_v4(),
                yaml: "phases: []".to_string(),
            }))
            .await
            .err()
            .unwrap_or_else(|| panic!("expected error"));
        assert_eq!(err.code, rmcp::model::ErrorCode::INVALID_PARAMS);
    }
}
