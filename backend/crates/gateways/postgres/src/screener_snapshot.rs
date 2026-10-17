use async_trait::async_trait;
use core_application::screener::ScreenerSnapshotRefresher;
use sea_orm::ConnectionTrait;

use crate::DatabaseHandle;
use crate::persistence::persistence_error;

const REFRESH_SCREENER_SNAPSHOT_SQL: &str =
    "REFRESH MATERIALIZED VIEW CONCURRENTLY screener_stock_snapshot";

#[derive(Clone)]
pub struct PostgresScreenerSnapshotRefresher {
    db: DatabaseHandle,
}

impl PostgresScreenerSnapshotRefresher {
    pub fn new(db: DatabaseHandle) -> Self {
        Self { db }
    }
}

#[async_trait]
impl ScreenerSnapshotRefresher for PostgresScreenerSnapshotRefresher {
    async fn refresh_snapshot(
        &self,
    ) -> Result<(), core_application::persistence::PersistenceError> {
        self.db
            .execute_unprepared(REFRESH_SCREENER_SNAPSHOT_SQL)
            .await
            .map(|_| ())
            .map_err(persistence_error)
    }
}
