use core_application::widget::{WidgetDefinition, WidgetQueryError, WidgetUseCaseError};
use rmcp::ErrorData as McpError;

use super::dto::{
    QueryWidgetGroupDto, QueryWidgetParams, QueryWidgetResult, QueryWidgetRowClassificationDto,
    QueryWidgetRowDto, QueryWidgetSeriesDto, QueryWidgetSeriesPointDto, QueryWidgetSeriesTargetDto,
};
use super::{StrategyServer, internal_error, invalid_params};

fn widget_use_case_to_mcp(error: WidgetUseCaseError) -> McpError {
    match error {
        WidgetUseCaseError::Validation(errors) => invalid_params(errors.to_string()),
        WidgetUseCaseError::NoData => {
            invalid_params("no widget data is available on or before the requested date")
        }
        WidgetUseCaseError::Query(WidgetQueryError::Database(error)) => {
            super::persistence_error_to_mcp(error)
        }
        WidgetUseCaseError::Query(WidgetQueryError::StockGroups(error)) => {
            tracing::error!(error = %error, "strategy mcp widget stock group membership query failed");
            internal_error(format!("stock group membership query failed: {error}"))
        }
        WidgetUseCaseError::Query(
            error @ (WidgetQueryError::IncompatibleCurrencies
            | WidgetQueryError::InputLimitExceeded { .. }),
        ) => invalid_params(error.to_string()),
        WidgetUseCaseError::Timeout => internal_error("widget query exceeded its time limit"),
        WidgetUseCaseError::Query(
            error @ (WidgetQueryError::Timeout | WidgetQueryError::SeriesUnsupported),
        ) => internal_error(error.to_string()),
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
                    classification: row.classification.map(|classification| {
                        QueryWidgetRowClassificationDto {
                            axis: classification.axis,
                            group: classification.group.map(|group| QueryWidgetGroupDto {
                                key: group.key,
                                name: group.name,
                            }),
                        }
                    }),
                })
                .collect(),
            series: response.series.map(|series| {
                series
                    .into_iter()
                    .map(|series| QueryWidgetSeriesDto {
                        key: series.key,
                        query: series.query,
                        name: series.name,
                        target: series.target.map(|target| QueryWidgetSeriesTargetDto {
                            key: target.key,
                            name: target.name,
                        }),
                        points: series
                            .points
                            .into_iter()
                            .map(|point| QueryWidgetSeriesPointDto {
                                date: point.date,
                                value: point.value,
                            })
                            .collect(),
                    })
                    .collect()
            }),
        })
    }
}
