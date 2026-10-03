//! 戦略実行 MCP の `read_macro_indicator` tool。

use core_application::indicator_observation::{
    IndicatorObservationQuery, IndicatorObservationUseCaseError,
};
use core_application::strategy_scope::StrategyScope;
use rmcp::ErrorData as McpError;

use super::dto::{IndicatorObservationDto, ReadMacroIndicatorParams, ReadMacroIndicatorResult};
use super::{StrategyServer, decimal_to_f64, internal_error, invalid_params};

impl StrategyServer {
    pub(crate) async fn read_macro_indicator_inner(
        &self,
        scope: impl Into<StrategyScope>,
        params: ReadMacroIndicatorParams,
    ) -> Result<ReadMacroIndicatorResult, McpError> {
        let result = self
            .dependencies
            .indicator_observations
            .read(
                scope.into(),
                IndicatorObservationQuery {
                    indicator_id: params.indicator_id,
                    from: params.from,
                    to: params.to,
                },
            )
            .await
            .map_err(indicator_observation_error)?;

        Ok(ReadMacroIndicatorResult {
            indicator_id: result.indicator_id,
            observations: result
                .observations
                .into_iter()
                .map(|observation| IndicatorObservationDto {
                    date: observation.date,
                    value: decimal_to_f64(observation.value),
                })
                .collect(),
        })
    }
}

fn indicator_observation_error(error: IndicatorObservationUseCaseError) -> McpError {
    match error {
        IndicatorObservationUseCaseError::Validation(message) => invalid_params(message),
        IndicatorObservationUseCaseError::Repository(error) => {
            tracing::error!(error = %error, "strategy mcp indicator observation read failed");
            internal_error(format!("database error: {error}"))
        }
    }
}
