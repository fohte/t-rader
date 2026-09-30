use std::sync::Arc;

use core_application::short_ratio::ShortRatioUseCases;
use gateway_postgres::PostgresShortRatioRepository;

use super::UseCases;

impl UseCases {
    pub fn short_ratios(&self) -> ShortRatioUseCases {
        ShortRatioUseCases::new(Arc::new(PostgresShortRatioRepository::new(self.db.clone())))
    }
}
