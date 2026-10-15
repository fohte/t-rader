use std::sync::Arc;

use core_application::ingest_run_log::IngestRunLogUseCases;
use gateway_postgres::{PostgresEventRepository, PostgresIngestRunRepository};

use super::UseCases;

impl UseCases {
    pub fn ingest_run_log(&self) -> IngestRunLogUseCases {
        IngestRunLogUseCases::new(
            self.unit_of_work.clone(),
            Arc::new(PostgresIngestRunRepository::new(self.db.clone())),
            Arc::new(PostgresEventRepository::new(self.db.clone())),
            self.event_dispatch_job(),
        )
    }
}
