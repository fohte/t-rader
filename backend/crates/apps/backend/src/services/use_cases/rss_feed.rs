use std::sync::Arc;

use core_application::rss_feed::RssFeedUseCases;
use gateway_postgres::PostgresRssFeedRepository;
use gateway_rss::HttpRssFeedUrlValidator;

use super::UseCases;

impl UseCases {
    pub fn rss_feeds(&self) -> RssFeedUseCases {
        RssFeedUseCases::new(
            self.unit_of_work.clone(),
            Arc::new(PostgresRssFeedRepository::new(self.db.clone())),
            Arc::new(HttpRssFeedUrlValidator),
        )
    }
}
