use std::sync::Arc;

use core_application::document_mentions::DocumentMentionUseCases;
use gateway_postgres::PostgresDocumentMentionRepository;

use super::UseCases;

impl UseCases {
    pub fn document_mentions(&self) -> DocumentMentionUseCases {
        DocumentMentionUseCases::new(Arc::new(PostgresDocumentMentionRepository::new(
            self.db.clone(),
        )))
    }
}
