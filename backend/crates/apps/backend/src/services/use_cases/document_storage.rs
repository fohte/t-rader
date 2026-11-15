use std::sync::Arc;

use core_application::document_storage::DocumentStorageUseCases;
use gateway_postgres::PostgresDocumentStorageRepository;

use super::UseCases;

impl UseCases {
    pub fn document_storage(&self) -> DocumentStorageUseCases {
        DocumentStorageUseCases::new(
            self.unit_of_work.clone(),
            Arc::new(PostgresDocumentStorageRepository::new(self.db.clone())),
        )
    }
}
