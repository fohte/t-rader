use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use core_application::dashboard::{DashboardConfig, DashboardConfigError, DashboardWidgetWidth};
use core_application::widget::{WidgetQueryError, WidgetUseCaseError};
use rmcp::ErrorData as McpError;

use super::dto::{
    DashboardConfigIssue, DashboardWidgetPreview, DashboardWidgetRowPreview,
    DashboardWidgetWidthPreview, PreviewDashboardParams, PreviewDashboardResult,
};
use super::{MgmtServer, internal_failure};

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
            Err(DashboardConfigError::WidgetParse { index, message }) => {
                return Ok(PreviewDashboardResult {
                    errors: vec![DashboardConfigIssue {
                        path: format!("widgets[{index}]"),
                        message,
                    }],
                    preview_url: None,
                    widgets: Vec::new(),
                });
            }
        };

        let mut widgets = Vec::with_capacity(config.widgets.len());
        let mut errors = Vec::new();
        for (index, widget) in config.widgets.into_iter().enumerate() {
            let result = self.dependencies.widgets.query(widget.definition).await;
            match result {
                Ok(result) => widgets.push(DashboardWidgetPreview {
                    title: widget.title,
                    width: match widget.width {
                        DashboardWidgetWidth::Half => DashboardWidgetWidthPreview::Half,
                        DashboardWidgetWidth::Full => DashboardWidgetWidthPreview::Full,
                    },
                    as_of: result.as_of,
                    rows: result
                        .rows
                        .into_iter()
                        .map(DashboardWidgetRowPreview::from)
                        .collect(),
                }),
                Err(WidgetUseCaseError::Validation(widget_errors)) => {
                    errors.extend(
                        widget_errors
                            .0
                            .into_iter()
                            .map(|error| DashboardConfigIssue {
                                path: format!("widgets[{index}].{}", error.path),
                                message: error.message,
                            }),
                    );
                }
                Err(WidgetUseCaseError::NoData) => errors.push(DashboardConfigIssue {
                    path: format!("widgets[{index}].time.as_of"),
                    message: "no widget data is available on or before the requested date"
                        .to_owned(),
                }),
                Err(WidgetUseCaseError::Query(WidgetQueryError::IncompatibleCurrencies)) => {
                    errors.push(DashboardConfigIssue {
                        path: format!("widgets[{index}].queries"),
                        message:
                            "amount aggregations across different currencies are not supported"
                                .to_owned(),
                    });
                }
                Err(WidgetUseCaseError::Query(error)) => {
                    return Err(internal_failure(&error.to_string()));
                }
            }
        }

        Ok(PreviewDashboardResult {
            errors,
            preview_url: Some(format!(
                "/dashboards/preview#{}",
                URL_SAFE_NO_PAD.encode(params.yaml)
            )),
            widgets,
        })
    }
}
