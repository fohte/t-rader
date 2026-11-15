use core_domain::document_search::prepare_document_search;

use super::repository::SharedDocumentSearchRepository;
use super::types::{DocumentSearchCriteria, DocumentSearchResult};

#[derive(Clone)]
pub struct DocumentSearchUseCases {
    repository: SharedDocumentSearchRepository,
}

impl DocumentSearchUseCases {
    pub fn new(repository: SharedDocumentSearchRepository) -> Self {
        Self { repository }
    }

    pub async fn search(
        &self,
        criteria: DocumentSearchCriteria,
    ) -> Result<Vec<DocumentSearchResult>, super::repository::DocumentSearchRepositoryError> {
        let query = prepare_document_search(&criteria.query);
        if query.normalized_text.trim().is_empty() || query.terms.is_empty() || criteria.limit == 0
        {
            return Ok(Vec::new());
        }
        self.repository
            .search(query, criteria.as_of, criteria.stock_id, criteria.limit)
            .await
    }
}
