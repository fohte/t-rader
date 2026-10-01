use thiserror::Error;
use uuid::Uuid;

use super::query::SharedChangeHistoryQuery;
use super::{ChangeHistoryEntry, ChangeHistoryListQuery, ChangeHistoryQueryError};

const DEFAULT_LIMIT: u64 = 100;
const MAX_LIMIT: u64 = 500;

#[derive(Debug, Error)]
pub enum ChangeHistoryUseCaseError {
    #[error("history {0} not found")]
    NotFound(Uuid),
    #[error(transparent)]
    Query(#[from] ChangeHistoryQueryError),
}

#[derive(Clone)]
pub struct ChangeHistoryUseCases {
    query: SharedChangeHistoryQuery,
}

impl ChangeHistoryUseCases {
    pub fn new(query: SharedChangeHistoryQuery) -> Self {
        Self { query }
    }

    pub async fn list(
        &self,
        target_kind: Option<&str>,
        target_id: Option<Uuid>,
        limit: Option<u64>,
    ) -> Result<Vec<ChangeHistoryEntry>, ChangeHistoryUseCaseError> {
        self.query
            .list(ChangeHistoryListQuery {
                target_kind: target_kind
                    .filter(|target_kind| !target_kind.is_empty())
                    .map(str::to_owned),
                target_id,
                limit: limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT),
            })
            .await
            .map_err(Into::into)
    }

    pub async fn get(&self, id: Uuid) -> Result<ChangeHistoryEntry, ChangeHistoryUseCaseError> {
        self.query
            .find_by_id(id)
            .await?
            .ok_or(ChangeHistoryUseCaseError::NotFound(id))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use async_trait::async_trait;
    use rstest::{fixture, rstest};

    use super::*;
    use crate::change_history::{ChangeHistoryQuery, ChangeHistoryQueryError};

    #[derive(Default)]
    struct FakeChangeHistoryQuery {
        list_queries: Mutex<Vec<ChangeHistoryListQuery>>,
    }

    #[async_trait]
    impl ChangeHistoryQuery for FakeChangeHistoryQuery {
        async fn list(
            &self,
            query: ChangeHistoryListQuery,
        ) -> Result<Vec<ChangeHistoryEntry>, ChangeHistoryQueryError> {
            self.list_queries
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .push(query);
            Ok(Vec::new())
        }

        async fn find_by_id(
            &self,
            _id: Uuid,
        ) -> Result<Option<ChangeHistoryEntry>, ChangeHistoryQueryError> {
            Ok(None)
        }
    }

    struct Fixture {
        use_cases: ChangeHistoryUseCases,
        query: Arc<FakeChangeHistoryQuery>,
    }

    #[fixture]
    fn fixture() -> Fixture {
        let query = Arc::new(FakeChangeHistoryQuery::default());
        Fixture {
            use_cases: ChangeHistoryUseCases::new(query.clone()),
            query,
        }
    }

    #[rstest]
    #[case::default_values(None, None, None, None, None, 100)]
    #[case::empty_kind_and_minimum_limit(
        Some(""),
        Some(Uuid::from_u128(1)),
        Some(0),
        None,
        Some(Uuid::from_u128(1)),
        1
    )]
    #[case::filters_and_maximum_limit(
        Some("fixture_kind"),
        None,
        Some(501),
        Some("fixture_kind"),
        None,
        500
    )]
    #[tokio::test]
    async fn list_normalizes_filters_and_limit(
        fixture: Fixture,
        #[case] target_kind: Option<&str>,
        #[case] target_id: Option<Uuid>,
        #[case] limit: Option<u64>,
        #[case] expected_target_kind: Option<&str>,
        #[case] expected_target_id: Option<Uuid>,
        #[case] expected_limit: u64,
    ) {
        let result = fixture
            .use_cases
            .list(target_kind, target_id, limit)
            .await
            .map_err(|error| error.to_string());
        let queries = fixture
            .query
            .list_queries
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();

        assert_eq!(
            (result, queries),
            (
                Ok(Vec::new()),
                vec![ChangeHistoryListQuery {
                    target_kind: expected_target_kind.map(str::to_owned),
                    target_id: expected_target_id,
                    limit: expected_limit,
                }],
            ),
        );
    }

    #[tokio::test]
    async fn get_returns_not_found_for_a_missing_history() {
        let fixture = fixture();
        let id = Uuid::from_u128(2);
        let result = fixture
            .use_cases
            .get(id)
            .await
            .map_err(|error| error.to_string());

        assert_eq!(result, Err(format!("history {id} not found")));
    }
}
