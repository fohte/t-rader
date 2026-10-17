use chrono::{DateTime, FixedOffset};
use core_application::dashboard::{Dashboard, DashboardData};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateDashboardRequest {
    pub key: String,
    pub config: String,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct UpdateDashboardRequest {
    pub config: String,
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(as = Dashboard)]
pub struct DashboardResponse {
    pub key: String,
    pub config: String,
    #[schema(value_type = chrono::DateTime<chrono::Utc>)]
    pub created_at: DateTime<FixedOffset>,
    #[schema(value_type = chrono::DateTime<chrono::Utc>)]
    pub updated_at: DateTime<FixedOffset>,
}

impl From<CreateDashboardRequest> for DashboardData {
    fn from(request: CreateDashboardRequest) -> Self {
        Self {
            key: request.key,
            config: request.config,
        }
    }
}

impl From<Dashboard> for DashboardResponse {
    fn from(dashboard: Dashboard) -> Self {
        Self {
            key: dashboard.key,
            config: dashboard.config,
            created_at: dashboard.created_at,
            updated_at: dashboard.updated_at,
        }
    }
}
