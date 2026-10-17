use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use core_application::dashboard::{DashboardConfig, DashboardConfigError, DashboardWidgetWidth};
use core_application::widget::{WidgetQueryError, WidgetUseCaseError};
use rmcp::ErrorData as McpError;

use super::dto::{
    DashboardConfigIssue, DashboardWidgetPreview, PreviewDashboardParams, PreviewDashboardResult,
};
use super::{MgmtServer, internal_failure, invalid_params};

impl MgmtServer {
    pub(super) async fn preview_dashboard_inner(
        &self,
        params: PreviewDashboardParams,
    ) -> Result<PreviewDashboardResult, McpError> {
        let config = match DashboardConfig::from_yaml(&params.yaml) {
            Ok(config) => config,
            Err(DashboardConfigError::Parse(error)) => {
                return Ok(PreviewDashboardResult {
                    errors: vec![DashboardConfigIssue {
                        path: "$".to_owned(),
                        message: error.to_string(),
                    }],
                    preview_url: None,
                    widgets: Vec::new(),
                });
            }
            Err(DashboardConfigError::Validation(errors)) => {
                return Ok(PreviewDashboardResult {
                    errors: errors
                        .0
                        .into_iter()
                        .map(|error| DashboardConfigIssue {
                            path: error.path,
                            message: error.message,
                        })
                        .collect(),
                    preview_url: None,
                    widgets: Vec::new(),
                });
            }
        };

        let mut widgets = Vec::with_capacity(config.widgets.len());
        for widget in config.widgets {
            let result = self
                .dependencies
                .widgets
                .query(widget.definition)
                .await
                .map_err(map_widget_error)?;
            widgets.push(DashboardWidgetPreview {
                title: widget.title,
                width: match widget.width {
                    DashboardWidgetWidth::Half => "half",
                    DashboardWidgetWidth::Full => "full",
                }
                .to_owned(),
                as_of: result.as_of,
                rows: result
                    .rows
                    .into_iter()
                    .map(|row| {
                        serde_json::json!({
                            "key": row.key.as_string(),
                            "name": row.name,
                            "values": row.values,
                        })
                    })
                    .collect(),
            });
        }

        Ok(PreviewDashboardResult {
            errors: Vec::new(),
            preview_url: Some(format!(
                "/dashboards/preview#{}",
                URL_SAFE_NO_PAD.encode(params.yaml)
            )),
            widgets,
        })
    }
}

fn map_widget_error(error: WidgetUseCaseError) -> McpError {
    match error {
        WidgetUseCaseError::Validation(errors) => invalid_params(errors.to_string()),
        WidgetUseCaseError::NoData => {
            invalid_params("no daily bar data is available on or before the requested date")
        }
        WidgetUseCaseError::Query(WidgetQueryError::Database(error)) => {
            internal_failure(&error.to_string())
        }
    }
}
