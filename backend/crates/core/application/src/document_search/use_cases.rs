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
        if !query.normalized_text.chars().any(char::is_alphanumeric) || criteria.limit == 0 {
            return Ok(Vec::new());
        }
        self.repository
            .search(query, criteria.as_of, criteria.stock_id, criteria.limit)
            .await
    }
}

#[cfg(all(test, feature = "test-support"))]
mod tests {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    use async_trait::async_trait;
    use chrono::{DateTime, FixedOffset};
    use rstest::{fixture, rstest};

    use crate::document_search::{
        DocumentSearchCriteria, DocumentSearchRepository, DocumentSearchRepositoryError,
        DocumentSearchResult, DocumentSearchUseCases,
    };
    use core_domain::document_search::PreparedDocumentSearch;

    #[derive(Default)]
    struct CountingRepository {
        search_calls: AtomicUsize,
    }

    #[async_trait]
    impl DocumentSearchRepository for CountingRepository {
        async fn search(
            &self,
            _query: PreparedDocumentSearch,
            _as_of: Option<DateTime<FixedOffset>>,
            _stock_id: Option<String>,
            _limit: u32,
        ) -> Result<Vec<DocumentSearchResult>, DocumentSearchRepositoryError> {
            self.search_calls.fetch_add(1, Ordering::Relaxed);
            Ok(Vec::new())
        }
    }

    #[fixture]
    fn repository() -> Arc<CountingRepository> {
        Arc::new(CountingRepository::default())
    }

    #[rstest]
    #[case::empty("", 10)]
    #[case::punctuation_only("!?", 10)]
    #[case::zero_limit("synthetic", 0)]
    #[tokio::test]
    async fn skips_search_when_criteria_cannot_match(
        #[case] query: &str,
        #[case] limit: u32,
        repository: Arc<CountingRepository>,
    ) {
        let use_cases = DocumentSearchUseCases::new(repository.clone());
        let result = use_cases
            .search(DocumentSearchCriteria {
                query: query.into(),
                as_of: None,
                stock_id: None,
                limit,
            })
            .await
            .map_err(|error| error.to_string());

        assert_eq!(
            (result, repository.search_calls.load(Ordering::Relaxed)),
            (Ok(Vec::new()), 0),
        );
    }
}
