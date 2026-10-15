use std::sync::Arc;

use core_application::group_movers::GroupMoversUseCases;
use gateway_postgres::PostgresGroupMoversQuerySource;

use super::UseCases;

impl UseCases {
    pub fn group_movers(&self) -> GroupMoversUseCases {
        GroupMoversUseCases::new(Arc::new(PostgresGroupMoversQuerySource::new(
            self.db.clone(),
        )))
    }
}
