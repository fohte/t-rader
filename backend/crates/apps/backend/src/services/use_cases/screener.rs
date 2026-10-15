use std::sync::Arc;

use core_application::screener::ScreenerUseCases;
use gateway_postgres::PostgresScreenerQuerySource;

use super::UseCases;

impl UseCases {
    pub fn screener(&self) -> ScreenerUseCases {
        ScreenerUseCases::new(Arc::new(PostgresScreenerQuerySource::new(
            self.db.clone(),
        )))
    }
}
