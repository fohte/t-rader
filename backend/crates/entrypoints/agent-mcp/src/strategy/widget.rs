use core_application::widget::{WidgetDefinition, WidgetQueryError, WidgetUseCaseError};
use rmcp::ErrorData as McpError;

use super::dto::{QueryWidgetParams, QueryWidgetResult, QueryWidgetRowDto};
use super::{StrategyServer, invalid_params};

fn widget_use_case_to_mcp(error: WidgetUseCaseError) -> McpError {
    match error {
        WidgetUseCaseError::Validation(errors) => invalid_params(errors.to_string()),
        WidgetUseCaseError::NoData => {
            invalid_params("no daily bar data is available on or before the requested date")
        }
        WidgetUseCaseError::Query(WidgetQueryError::Database(error)) => {
            super::persistence_error_to_mcp(error)
        }
    }
}

impl StrategyServer {
    pub(crate) async fn query_widget_inner(
        &self,
        params: QueryWidgetParams,
    ) -> Result<QueryWidgetResult, McpError> {
        let definition = WidgetDefinition::from_yaml(&params.definition_yaml)
            .map_err(|error| invalid_params(error.to_string()))?;
        let response = self
            .dependencies
            .widgets
            .query(definition)
            .await
            .map_err(widget_use_case_to_mcp)?;

        Ok(QueryWidgetResult {
            as_of: response.as_of,
            rows: response
                .rows
                .into_iter()
                .map(|row| QueryWidgetRowDto {
                    key: row.key.as_string(),
                    name: row.name,
                    values: row.values,
                })
                .collect(),
        })
    }
}
