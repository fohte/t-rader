use std::sync::Arc;

use gateway_postgres::{PostgresEventRepository, PostgresIngestRunLog};

use super::UseCases;

impl UseCases {
    pub fn ingest_run_log(&self) -> PostgresIngestRunLog {
        PostgresIngestRunLog::new(
            self.db.clone(),
            self.unit_of_work.clone(),
            Arc::new(PostgresEventRepository::new(self.db.clone())),
            self.event_dispatch_job(),
        )
    }
}
