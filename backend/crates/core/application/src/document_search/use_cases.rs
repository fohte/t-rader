use core_domain::document_search::prepare_document_search;
use thiserror::Error;

use super::repository::SharedDocumentSearchRepository;
use super::types::{DocumentSearchCriteria, DocumentSearchResult};

const MAX_SEARCH_LIMIT: u32 = 100;

#[derive(Debug, Error)]
pub enum DocumentSearchUseCaseError {
    #[error("query must contain at least one word with 2 or more characters")]
    InvalidQuery,
    #[error("limit must be between 1 and 100")]
    InvalidLimit,
    #[error(transparent)]
    Repository(#[from] super::repository::DocumentSearchRepositoryError),
}

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
    ) -> Result<Vec<DocumentSearchResult>, DocumentSearchUseCaseError> {
        let query = prepare_document_search(&criteria.query);
        if query.phrases.is_empty()
            || query
                .phrases
                .iter()
                .any(|phrase| phrase.chars().count() < 2)
        {
            return Err(DocumentSearchUseCaseError::InvalidQuery);
        }
        if criteria.limit == 0 || criteria.limit > MAX_SEARCH_LIMIT {
            return Err(DocumentSearchUseCaseError::InvalidLimit);
        }
        self.repository
            .search(query, criteria.as_of, criteria.stock_id, criteria.limit)
            .await
            .map_err(Into::into)
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
    #[case::valid("  Synthetic\t phrase  ", 100, None, 1)]
    #[case::empty_query(
        "",
        10,
        Some("query must contain at least one word with 2 or more characters"),
        0
    )]
    #[case::whitespace_only(
        " \t\n",
        10,
        Some("query must contain at least one word with 2 or more characters"),
        0
    )]
    #[case::single_character_query(
        "語",
        10,
        Some("query must contain at least one word with 2 or more characters"),
        0
    )]
    #[case::single_character_word(
        "synthetic 語",
        10,
        Some("query must contain at least one word with 2 or more characters"),
        0
    )]
    #[case::zero_limit("synthetic phrase", 0, Some("limit must be between 1 and 100"), 0)]
    #[case::limit_above_maximum(
        "synthetic phrase",
        101,
        Some("limit must be between 1 and 100"),
        0
    )]
    #[tokio::test]
    async fn validates_search_input_before_querying(
        #[case] query: &str,
        #[case] limit: u32,
        #[case] expected_error: Option<&str>,
        #[case] expected_calls: usize,
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
            (
                expected_error.map_or_else(|| Ok(Vec::new()), |message| Err(message.into())),
                expected_calls,
            ),
        );
    }
}
