use std::sync::Arc;

use core_application::screener::{ScreenerSnapshotUseCases, ScreenerUseCases};
use gateway_postgres::{PostgresScreenerQuerySource, PostgresScreenerSnapshotRefresher};

use super::UseCases;

impl UseCases {
    pub fn screener(&self) -> ScreenerUseCases {
        ScreenerUseCases::new(Arc::new(PostgresScreenerQuerySource::new(
            self.db.clone(),
        )))
    }

    pub fn screener_snapshot(&self) -> ScreenerSnapshotUseCases {
        ScreenerSnapshotUseCases::new(Arc::new(PostgresScreenerSnapshotRefresher::new(
            self.db.clone(),
        )))
    }
}
