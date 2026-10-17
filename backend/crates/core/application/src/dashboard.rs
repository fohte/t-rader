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

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DashboardConfig {
    pub title: String,
    pub widgets: Vec<DashboardWidget>,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
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
    #[error("{0}")]
    Validation(#[from] DashboardValidationErrors),
}

impl DashboardConfig {
    pub fn from_yaml(input: &str) -> Result<Self, DashboardConfigError> {
        let value: serde_yaml_ng::Value = serde_yaml_ng::from_str(input)?;
        let config: Self = serde_yaml_ng::from_value(value.clone())?;
        config.validate()?;
        let unknown_fields = unknown_widget_fields(&value);
        if !unknown_fields.is_empty() {
            return Err(DashboardValidationErrors(unknown_fields).into());
        }
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

fn unknown_widget_fields(value: &serde_yaml_ng::Value) -> Vec<DashboardValidationError> {
    // Serde の flatten は内部型の deny_unknown_fields で余剰キーを拒否できないため、元の YAML を確認する。
    let serde_yaml_ng::Value::Mapping(root) = value else {
        return Vec::new();
    };
    let widgets_key = serde_yaml_ng::Value::String("widgets".to_owned());
    let Some(serde_yaml_ng::Value::Sequence(widgets)) = root.get(&widgets_key) else {
        return Vec::new();
    };

    widgets
        .iter()
        .enumerate()
        .flat_map(|(index, widget)| {
            let serde_yaml_ng::Value::Mapping(fields) = widget else {
                return Vec::new();
            };
            fields
                .keys()
                .filter_map(|key| {
                    let field = key.as_str().unwrap_or("<non-string>");
                    (!matches!(
                        field,
                        "title" | "width" | "queries" | "formulas" | "viz" | "time"
                    ))
                    .then(|| DashboardValidationError {
                        path: format!("widgets[{index}].{field}"),
                        message: "unknown field".to_owned(),
                    })
                })
                .collect::<Vec<_>>()
        })
        .collect()
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
    use indoc::indoc;

    use super::{DashboardConfig, DashboardValidationError};

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
            .map_err(|_| ())
            .map(|config| config.widgets[0].title.clone()),
            Ok("sample-widget".to_owned()),
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
            .map(|_| Vec::<DashboardValidationError>::new())
            .map_err(|error| match error {
                super::DashboardConfigError::Validation(errors) => errors.0,
                super::DashboardConfigError::Parse(_) => Vec::new(),
            }),
            Err(vec![DashboardValidationError {
                path: "widgets[0].unknown".to_owned(),
                message: "unknown field".to_owned(),
            }]),
        );
    }
}
