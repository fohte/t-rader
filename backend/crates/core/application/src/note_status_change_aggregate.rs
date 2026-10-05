use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, FixedOffset};
use thiserror::Error;

use crate::persistence::PersistenceError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoteStatusChangePeriod {
    pub from: DateTime<FixedOffset>,
    pub to: DateTime<FixedOffset>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NoteStatusChangeCounts {
    pub approved: i64,
    pub rejected: i64,
}

#[derive(Debug, Error)]
pub enum NoteStatusChangeAggregateQueryError {
    #[error(transparent)]
    Database(#[from] PersistenceError),
}

#[async_trait]
pub trait NoteStatusChangeAggregateQuery: Send + Sync {
    async fn count(
        &self,
        period: NoteStatusChangePeriod,
    ) -> Result<NoteStatusChangeCounts, NoteStatusChangeAggregateQueryError>;
}

pub type SharedNoteStatusChangeAggregateQuery =
    Arc<dyn NoteStatusChangeAggregateQuery + Send + Sync>;

#[derive(Debug, Error)]
pub enum NoteStatusChangeAggregateUseCaseError {
    #[error("period end must be later than period start")]
    InvalidPeriod,
    #[error(transparent)]
    Query(#[from] NoteStatusChangeAggregateQueryError),
}

#[derive(Clone)]
pub struct NoteStatusChangeAggregateUseCases {
    query: SharedNoteStatusChangeAggregateQuery,
}

impl NoteStatusChangeAggregateUseCases {
    pub fn new(query: SharedNoteStatusChangeAggregateQuery) -> Self {
        Self { query }
    }

    pub async fn count(
        &self,
        period: NoteStatusChangePeriod,
    ) -> Result<NoteStatusChangeCounts, NoteStatusChangeAggregateUseCaseError> {
        if period.from >= period.to {
            return Err(NoteStatusChangeAggregateUseCaseError::InvalidPeriod);
        }

        self.query.count(period).await.map_err(Into::into)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, reason = "テスト用の固定日時を構築するため")]

    use std::sync::{Arc, Mutex};

    use rstest::{fixture, rstest};

    use super::*;

    #[derive(Default)]
    struct FakeNoteStatusChangeAggregateQuery {
        periods: Mutex<Vec<NoteStatusChangePeriod>>,
    }

    #[async_trait]
    impl NoteStatusChangeAggregateQuery for FakeNoteStatusChangeAggregateQuery {
        async fn count(
            &self,
            period: NoteStatusChangePeriod,
        ) -> Result<NoteStatusChangeCounts, NoteStatusChangeAggregateQueryError> {
            self.periods
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .push(period);
            Ok(NoteStatusChangeCounts {
                approved: 3,
                rejected: 2,
            })
        }
    }

    struct Fixture {
        use_cases: NoteStatusChangeAggregateUseCases,
        query: Arc<FakeNoteStatusChangeAggregateQuery>,
    }

    #[fixture]
    fn fixture() -> Fixture {
        let query = Arc::new(FakeNoteStatusChangeAggregateQuery::default());
        Fixture {
            use_cases: NoteStatusChangeAggregateUseCases::new(query.clone()),
            query,
        }
    }

    fn timestamp(value: &str) -> DateTime<FixedOffset> {
        value.parse().expect("timestamp")
    }

    #[rstest]
    #[case::same_instants("2026-01-01T00:00:00Z", "2026-01-01T00:00:00Z")]
    #[case::reversed_period("2026-01-02T00:00:00Z", "2026-01-01T00:00:00Z")]
    #[tokio::test]
    async fn count_rejects_non_positive_periods(
        fixture: Fixture,
        #[case] from: &str,
        #[case] to: &str,
    ) {
        let result = fixture
            .use_cases
            .count(NoteStatusChangePeriod {
                from: timestamp(from),
                to: timestamp(to),
            })
            .await
            .map_err(|error| error.to_string());
        let queried_periods = fixture
            .query
            .periods
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();

        assert_eq!(
            (result, queried_periods),
            (
                Err("period end must be later than period start".into()),
                Vec::new(),
            ),
        );
    }

    #[rstest]
    #[tokio::test]
    async fn count_forwards_valid_period_and_returns_counts(fixture: Fixture) {
        let period = NoteStatusChangePeriod {
            from: timestamp("2026-01-01T00:00:00Z"),
            to: timestamp("2026-02-01T00:00:00Z"),
        };
        let result = fixture
            .use_cases
            .count(period.clone())
            .await
            .map_err(|error| error.to_string());
        let queried_periods = fixture
            .query
            .periods
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();

        assert_eq!(
            (result, queried_periods),
            (
                Ok(NoteStatusChangeCounts {
                    approved: 3,
                    rejected: 2,
                }),
                vec![period],
            ),
        );
    }
}
