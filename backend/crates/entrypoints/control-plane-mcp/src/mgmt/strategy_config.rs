//! 管理 MCP の戦略設定 (name / description) と trigger 一覧の取得 tool。
//!
//! HTTP handler と同じ application usecase を呼び出す。

use rmcp::ErrorData as McpError;

use super::dto::{GetStrategyConfigParams, GetStrategyConfigResult, TriggerSummary};
use super::{MgmtServer, map_strategy_use_case_error, map_trigger_error};

impl MgmtServer {
    pub(super) async fn get_strategy_config_inner(
        &self,
        params: GetStrategyConfigParams,
    ) -> Result<GetStrategyConfigResult, McpError> {
        let scope = self.strategy_scope(params.strategy_id).await?;
        let row = self
            .dependencies
            .strategies
            .get(scope)
            .await
            .map_err(map_strategy_use_case_error)?;
        let triggers = self
            .dependencies
            .triggers
            .list_for_strategy(scope, None)
            .await
            .map_err(map_trigger_error)?;
        Ok(GetStrategyConfigResult {
            strategy_id: row.id,
            name: row.name,
            description: row.description,
            triggers: triggers.into_iter().map(TriggerSummary::from).collect(),
        })
    }
}
