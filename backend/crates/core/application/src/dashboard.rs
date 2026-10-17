use std::sync::Arc;

use chrono::{DateTime, FixedOffset};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::persistence::PersistenceError;
use crate::widget::WidgetDefinition;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dashboard {
    pub key: String,
    pub config: String,
    pub created_at: DateTime<FixedOffset>,
    pub updated_at: DateTime<FixedOffset>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DashboardData {
    pub key: String,
    pub config: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DashboardConfig {
    pub title: String,
    pub widgets: Vec<DashboardWidget>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DashboardWidget {
    pub title: String,
    pub width: DashboardWidgetWidth,
    #[serde(flatten)]
    pub definition: WidgetDefinition,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DashboardWidgetWidth {
    Half,
    Full,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DashboardValidationError {
    pub path: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DashboardValidationErrors(pub Vec<DashboardValidationError>);

impl std::fmt::Display for DashboardValidationErrors {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let message = self
            .0
            .iter()
            .map(|error| format!("{}: {}", error.path, error.message))
            .collect::<Vec<_>>()
            .join("; ");
        formatter.write_str(&message)
    }
}

impl std::error::Error for DashboardValidationErrors {}

#[derive(Debug, Error)]
pub enum DashboardConfigError {
    #[error("invalid dashboard YAML: {0}")]
    Parse(#[from] serde_yaml_ng::Error),
    #[error("invalid dashboard widget at widgets[{index}]: {message}")]
    WidgetParse { index: usize, message: String },
    #[error("{0}")]
    Validation(#[from] DashboardValidationErrors),
}

impl DashboardConfig {
    pub fn from_yaml(input: &str) -> Result<Self, DashboardConfigError> {
        let raw: RawDashboardConfig = serde_yaml_ng::from_str(input)?;
        let widgets = raw
            .widgets
            .into_iter()
            .enumerate()
            .map(parse_dashboard_widget)
            .collect::<Result<Vec<_>, _>>()?;
        let config = Self {
            title: raw.title,
            widgets,
        };
        config.validate()?;
        Ok(config)
    }

    pub fn validate(&self) -> Result<(), DashboardValidationErrors> {
        let mut errors = Vec::new();
        if self.title.trim().is_empty() {
            errors.push(DashboardValidationError {
                path: "title".to_owned(),
                message: "title must not be empty".to_owned(),
            });
        }

        for (index, widget) in self.widgets.iter().enumerate() {
            let path = format!("widgets[{index}]");
            if widget.title.trim().is_empty() {
                errors.push(DashboardValidationError {
                    path: format!("{path}.title"),
                    message: "title must not be empty".to_owned(),
                });
            }
            if let Err(widget_errors) = widget.definition.validate() {
                errors.extend(
                    widget_errors
                        .0
                        .into_iter()
                        .map(|error| DashboardValidationError {
                            path: format!("{path}.{}", error.path),
                            message: error.message,
                        }),
                );
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(DashboardValidationErrors(errors))
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawDashboardConfig {
    title: String,
    widgets: Vec<serde_yaml_ng::Value>,
}

fn parse_dashboard_widget(
    (index, value): (usize, serde_yaml_ng::Value),
) -> Result<DashboardWidget, DashboardConfigError> {
    let serde_yaml_ng::Value::Mapping(mut fields) = value else {
        return Err(DashboardConfigError::WidgetParse {
            index,
            message: "expected a mapping".to_owned(),
        });
    };

    let title_key = serde_yaml_ng::Value::String("title".to_owned());
    let title = fields
        .remove(&title_key)
        .ok_or_else(|| DashboardConfigError::WidgetParse {
            index,
            message: "missing field `title`".to_owned(),
        })?;
    let title =
        serde_yaml_ng::from_value(title).map_err(|error| DashboardConfigError::WidgetParse {
            index,
            message: error.to_string(),
        })?;

    let width_key = serde_yaml_ng::Value::String("width".to_owned());
    let width = fields
        .remove(&width_key)
        .ok_or_else(|| DashboardConfigError::WidgetParse {
            index,
            message: "missing field `width`".to_owned(),
        })?;
    let width =
        serde_yaml_ng::from_value(width).map_err(|error| DashboardConfigError::WidgetParse {
            index,
            message: error.to_string(),
        })?;

    let definition =
        serde_yaml_ng::from_value(serde_yaml_ng::Value::Mapping(fields)).map_err(|error| {
            DashboardConfigError::WidgetParse {
                index,
                message: error.to_string(),
            }
        })?;

    Ok(DashboardWidget {
        title,
        width,
        definition,
    })
}

#[derive(Debug, Error)]
pub enum DashboardRepositoryError {
    #[error(transparent)]
    Persistence(#[from] PersistenceError),
    #[error("dashboard with key '{0}' already exists")]
    DuplicateKey(String),
}

#[async_trait::async_trait]
pub trait DashboardRepository: Send + Sync {
    async fn list(&self) -> Result<Vec<Dashboard>, DashboardRepositoryError>;
    async fn get(&self, key: &str) -> Result<Option<Dashboard>, DashboardRepositoryError>;
    async fn create(&self, data: DashboardData) -> Result<Dashboard, DashboardRepositoryError>;
    async fn update(
        &self,
        key: &str,
        config: String,
    ) -> Result<Option<Dashboard>, DashboardRepositoryError>;
    async fn delete(&self, key: &str) -> Result<bool, DashboardRepositoryError>;
}

pub type SharedDashboardRepository = Arc<dyn DashboardRepository + Send + Sync>;

#[derive(Debug, Error)]
pub enum DashboardUseCaseError {
    #[error("{0}")]
    Validation(String),
    #[error("dashboard '{0}' not found")]
    NotFound(String),
    #[error("{0}")]
    Conflict(String),
    #[error(transparent)]
    Repository(#[from] DashboardRepositoryError),
}

#[derive(Clone)]
pub struct DashboardUseCases {
    repository: SharedDashboardRepository,
}

impl DashboardUseCases {
    pub fn new(repository: SharedDashboardRepository) -> Self {
        Self { repository }
    }

    pub async fn list(&self) -> Result<Vec<Dashboard>, DashboardUseCaseError> {
        self.repository.list().await.map_err(Into::into)
    }

    pub async fn get(&self, key: &str) -> Result<Dashboard, DashboardUseCaseError> {
        self.repository
            .get(key)
            .await?
            .ok_or_else(|| DashboardUseCaseError::NotFound(key.to_owned()))
    }

    pub async fn create(&self, data: DashboardData) -> Result<Dashboard, DashboardUseCaseError> {
        let data = validate_data(data)?;
        self.repository
            .create(data)
            .await
            .map_err(map_repository_error)
    }

    pub async fn update(
        &self,
        key: &str,
        config: String,
    ) -> Result<Dashboard, DashboardUseCaseError> {
        let config = validate_config(&config)?;
        self.repository
            .update(key, config)
            .await?
            .ok_or_else(|| DashboardUseCaseError::NotFound(key.to_owned()))
    }

    pub async fn delete(&self, key: &str) -> Result<(), DashboardUseCaseError> {
        if self.repository.delete(key).await? {
            Ok(())
        } else {
            Err(DashboardUseCaseError::NotFound(key.to_owned()))
        }
    }
}

fn validate_data(mut data: DashboardData) -> Result<DashboardData, DashboardUseCaseError> {
    data.key = data.key.trim().to_owned();
    if data.key.is_empty() {
        return Err(DashboardUseCaseError::Validation(
            "key must not be empty".to_owned(),
        ));
    }
    data.config = validate_config(&data.config)?;
    Ok(data)
}

fn validate_config(config: &str) -> Result<String, DashboardUseCaseError> {
    DashboardConfig::from_yaml(config)
        .map(|_| config.to_owned())
        .map_err(|error| DashboardUseCaseError::Validation(error.to_string()))
}

fn map_repository_error(error: DashboardRepositoryError) -> DashboardUseCaseError {
    match error {
        DashboardRepositoryError::DuplicateKey(key) => {
            DashboardUseCaseError::Conflict(format!("dashboard '{key}' already exists"))
        }
        error => DashboardUseCaseError::Repository(error),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use indoc::indoc;

    use crate::widget::{
        WidgetBy, WidgetDefinition, WidgetMetric, WidgetPeriod, WidgetQuery, WidgetTime,
        WidgetUniverse, WidgetVisualization, WidgetVisualizationOrder,
    };

    use super::{DashboardConfig, DashboardValidationError, DashboardWidget, DashboardWidgetWidth};

    #[test]
    fn parses_a_dashboard_with_flat_widget_definitions() {
        assert_eq!(
            DashboardConfig::from_yaml(indoc! {"
                title: sample-dashboard
                widgets:
                  - title: sample-widget
                    width: half
                    queries:
                      movement:
                        metric: change
                        period: 1d
                        by: stock
                        universe:
                          stocks: [fictional-code]
                    viz:
                      type: toplist
                      value: movement
                      order: desc
                      limit: 5
            "})
            .map_err(|_| ()),
            Ok(DashboardConfig {
                title: "sample-dashboard".to_owned(),
                widgets: vec![DashboardWidget {
                    title: "sample-widget".to_owned(),
                    width: DashboardWidgetWidth::Half,
                    definition: WidgetDefinition {
                        queries: BTreeMap::from([(
                            "movement".to_owned(),
                            WidgetQuery {
                                metric: WidgetMetric::Change,
                                period: Some(WidgetPeriod::OneDay),
                                by: WidgetBy::Stock,
                                axis: None,
                                indicator: None,
                                universe: Some(WidgetUniverse::Stocks {
                                    stocks: vec!["fictional-code".to_owned()],
                                }),
                                agg: None,
                            },
                        )]),
                        formulas: BTreeMap::new(),
                        viz: WidgetVisualization::Toplist {
                            value: "movement".to_owned(),
                            order: WidgetVisualizationOrder::Desc,
                            limit: 5,
                        },
                        time: WidgetTime::default(),
                    },
                }],
            }),
        );
    }

    #[test]
    fn reports_dashboard_and_widget_validation_errors() {
        assert_eq!(
            DashboardConfig::from_yaml(indoc! {"
                title: sample-dashboard
                widgets:
                  - title: '  '
                    width: half
                    queries: {}
                    viz:
                      type: toplist
                      value: missing
                      order: desc
                      limit: 1
            "})
            .map(|_| Vec::<DashboardValidationError>::new())
            .map_err(|error| match error {
                super::DashboardConfigError::Validation(errors) => errors.0,
                super::DashboardConfigError::Parse(_) => Vec::new(),
                super::DashboardConfigError::WidgetParse { index, message } => {
                    vec![DashboardValidationError {
                        path: format!("widgets[{index}]"),
                        message,
                    }]
                }
            }),
            Err(vec![
                DashboardValidationError {
                    path: "widgets[0].title".to_owned(),
                    message: "title must not be empty".to_owned(),
                },
                DashboardValidationError {
                    path: "widgets[0].queries".to_owned(),
                    message: "at least one query is required".to_owned(),
                },
                DashboardValidationError {
                    path: "widgets[0].viz.value".to_owned(),
                    message: "unknown query or formula name: `missing`".to_owned(),
                },
            ]),
        );
    }

    #[test]
    fn rejects_unknown_widget_fields() {
        assert_eq!(
            DashboardConfig::from_yaml(indoc! {"
                title: sample-dashboard
                widgets:
                  - title: sample-widget
                    width: half
                    unknown: value
                    queries:
                      movement:
                        metric: change
                        period: 1d
                        by: stock
                    viz:
                      type: toplist
                      value: movement
                      order: desc
                      limit: 5
            "})
            .map_err(|error| match error {
                super::DashboardConfigError::WidgetParse { index, message } => (index, message),
                super::DashboardConfigError::Parse(error) => (usize::MAX, error.to_string()),
                super::DashboardConfigError::Validation(errors) => {
                    (usize::MAX, errors.to_string())
                }
            }),
            Err((
                0,
                "unknown field `unknown`, expected one of `queries`, `formulas`, `viz`, `time`"
                    .to_owned(),
            )),
        );
    }
}
