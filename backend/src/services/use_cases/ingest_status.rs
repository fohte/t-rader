use std::sync::Arc;

use core_application::ingest_status::IngestStatusUseCase;
use gateway_postgres::PostgresIngestStatusRepository;

use super::UseCases;

impl UseCases {
    pub fn ingest_status(&self) -> IngestStatusUseCase {
        IngestStatusUseCase::new(Arc::new(PostgresIngestStatusRepository::new(
            self.db.clone(),
        )))
    }
}
