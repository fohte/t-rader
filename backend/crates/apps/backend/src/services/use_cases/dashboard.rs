use std::sync::Arc;

use core_application::dashboard::DashboardUseCases;
use gateway_postgres::PostgresDashboardRepository;

use super::UseCases;

impl UseCases {
    pub fn dashboards(&self) -> DashboardUseCases {
        DashboardUseCases::new(Arc::new(PostgresDashboardRepository::new(self.db.clone())))
    }
}
