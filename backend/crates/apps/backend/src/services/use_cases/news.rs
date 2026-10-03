use std::sync::Arc;

use core_application::news::NewsUseCases;
use gateway_postgres::{PostgresNewsItemRepository, PostgresRssFeedRepository};

use super::UseCases;

impl UseCases {
    pub fn news(&self) -> NewsUseCases {
        NewsUseCases::new(
            self.unit_of_work.clone(),
            Arc::new(PostgresRssFeedRepository::new(self.db.clone())),
            Arc::new(PostgresNewsItemRepository::new(self.db.clone())),
        )
    }
}
