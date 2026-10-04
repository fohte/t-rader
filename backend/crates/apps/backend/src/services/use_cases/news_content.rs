use std::sync::Arc;

use core_application::news_content::NewsContentUseCases;
use gateway_postgres::PostgresNewsContentRepository;

use super::UseCases;

impl UseCases {
    pub fn news_content(&self) -> NewsContentUseCases {
        NewsContentUseCases::new(Arc::new(PostgresNewsContentRepository::new(
            self.db.clone(),
        )))
    }
}
