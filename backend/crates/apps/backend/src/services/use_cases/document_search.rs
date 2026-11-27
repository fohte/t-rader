use std::sync::Arc;

use core_application::document_search::DocumentSearchUseCases;
use gateway_postgres::PostgresDocumentSearchRepository;

use super::UseCases;

impl UseCases {
    pub fn document_search(&self) -> DocumentSearchUseCases {
        DocumentSearchUseCases::new(Arc::new(PostgresDocumentSearchRepository::new(
            self.db.clone(),
        )))
    }
}
