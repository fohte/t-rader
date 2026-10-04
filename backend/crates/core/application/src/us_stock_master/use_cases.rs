use serde::Serialize;

use crate::unit_of_work::SharedUnitOfWork;
use crate::us_stock_master::repository::SharedUsStockMasterRepository;
use crate::us_stock_master_source::UsStockMasterSource;

use super::error::UsStockMasterUseCaseError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
pub struct UsStockMasterSyncStats {
    pub stocks_upserted: usize,
}

#[derive(Clone)]
pub struct UsStockMasterUseCases {
    unit_of_work: SharedUnitOfWork,
    repository: SharedUsStockMasterRepository,
}

impl UsStockMasterUseCases {
    pub fn new(unit_of_work: SharedUnitOfWork, repository: SharedUsStockMasterRepository) -> Self {
        Self {
            unit_of_work,
            repository,
        }
    }

    pub async fn sync(
        &self,
        source: &dyn UsStockMasterSource,
    ) -> Result<UsStockMasterSyncStats, UsStockMasterUseCaseError> {
        let entries = source.fetch_all_us_stocks().await?;
        if entries.is_empty() {
            return Ok(UsStockMasterSyncStats::default());
        }

        let transaction = self.unit_of_work.begin().await?;
        let stocks_upserted = self.repository.upsert(&transaction, &entries).await?;
        self.unit_of_work.commit(transaction).await?;
        Ok(UsStockMasterSyncStats { stocks_upserted })
    }
}

#[cfg(all(test, feature = "test-support"))]
mod tests {
    use std::sync::Arc;

    use async_trait::async_trait;
    use core_domain::stock_id::{ForeignStockId, ForeignStockIdError};
    use core_domain::us_stock_master::UsStockMasterEntry;
    use rstest::{fixture, rstest};
    use tokio::sync::Mutex;

    use crate::persistence::PersistenceError;
    use crate::unit_of_work::{FakeUnitOfWork, SharedUnitOfWork, UnitOfWorkTransaction};
    use crate::us_stock_master::repository::{
        SharedUsStockMasterRepository, UsStockMasterRepository, UsStockMasterRepositoryError,
    };
    use crate::us_stock_master_source::{UsStockMasterSource, UsStockMasterSourceError};

    use super::{UsStockMasterSyncStats, UsStockMasterUseCases};

    struct Harness {
        use_cases: UsStockMasterUseCases,
        unit_of_work: Arc<FakeUnitOfWork>,
        repository: Arc<FakeRepository>,
    }

    #[fixture]
    fn harness() -> Harness {
        let unit_of_work = Arc::new(FakeUnitOfWork::new());
        let repository = Arc::new(FakeRepository::default());
        let unit_of_work_shared: SharedUnitOfWork = unit_of_work.clone();
        let repository_shared: SharedUsStockMasterRepository = repository.clone();
        let use_cases = UsStockMasterUseCases::new(unit_of_work_shared, repository_shared);
        Harness {
            use_cases,
            unit_of_work,
            repository,
        }
    }

    #[derive(Default)]
    struct FakeSource {
        entries: Vec<UsStockMasterEntry>,
        error: Option<String>,
        calls: Mutex<usize>,
    }

    #[async_trait]
    impl UsStockMasterSource for FakeSource {
        async fn fetch_all_us_stocks(
            &self,
        ) -> Result<Vec<UsStockMasterEntry>, UsStockMasterSourceError> {
            *self.calls.lock().await += 1;
            if let Some(error) = &self.error {
                return Err(UsStockMasterSourceError::Failed(error.clone()));
            }
            Ok(self.entries.clone())
        }
    }

    #[derive(Default)]
    struct FakeRepository {
        upsert_calls: Mutex<usize>,
        upsert_error: Mutex<Option<String>>,
    }

    #[async_trait]
    impl UsStockMasterRepository for FakeRepository {
        async fn upsert(
            &self,
            _transaction: &UnitOfWorkTransaction,
            entries: &[UsStockMasterEntry],
        ) -> Result<usize, UsStockMasterRepositoryError> {
            *self.upsert_calls.lock().await += 1;
            if let Some(error) = self.upsert_error.lock().await.clone() {
                return Err(UsStockMasterRepositoryError::Database(
                    PersistenceError::Database(error),
                ));
            }
            Ok(entries.len())
        }
    }

    fn stock_entry() -> Result<UsStockMasterEntry, ForeignStockIdError> {
        Ok(UsStockMasterEntry {
            id: ForeignStockId::new("US", "QZ7")?,
            name: "架空銘柄".into(),
            exchange: Some("架空市場".into()),
        })
    }

    #[rstest]
    #[tokio::test]
    async fn sync_skips_the_transaction_when_the_source_is_empty(harness: Harness) {
        let source = FakeSource::default();

        let stats = harness
            .use_cases
            .sync(&source)
            .await
            .expect("sync succeeds");

        let calls = *source.calls.lock().await;
        let upsert_calls = *harness.repository.upsert_calls.lock().await;
        let begun = harness.unit_of_work.begun.lock().await.len();
        let committed = harness.unit_of_work.committed.lock().await.len();
        assert_eq!(
            (stats, calls, upsert_calls, begun, committed),
            (UsStockMasterSyncStats::default(), 1, 0, 0, 0),
        );
    }

    #[rstest]
    #[tokio::test]
    async fn sync_stops_before_upsert_when_source_fetch_fails(harness: Harness) {
        let source = FakeSource {
            error: Some("synthetic failure".into()),
            ..FakeSource::default()
        };

        let result = harness
            .use_cases
            .sync(&source)
            .await
            .map_err(|error| error.to_string());
        let upsert_calls = *harness.repository.upsert_calls.lock().await;
        let begun = harness.unit_of_work.begun.lock().await.len();
        let committed = harness.unit_of_work.committed.lock().await.len();

        assert_eq!(
            (result, upsert_calls, begun, committed),
            (
                Err("US stock master source error: synthetic failure".into()),
                0,
                0,
                0,
            ),
        );
    }

    #[rstest]
    #[tokio::test]
    async fn sync_does_not_commit_when_repository_upsert_fails(harness: Harness) {
        let source = FakeSource {
            entries: vec![stock_entry().expect("synthetic stock entry is valid")],
            ..FakeSource::default()
        };
        *harness.repository.upsert_error.lock().await = Some("synthetic failure".into());

        let result = harness
            .use_cases
            .sync(&source)
            .await
            .map_err(|error| error.to_string());
        let upsert_calls = *harness.repository.upsert_calls.lock().await;
        let begun = harness.unit_of_work.begun.lock().await.len();
        let committed = harness.unit_of_work.committed.lock().await.len();

        assert_eq!(
            (result, upsert_calls, begun, committed),
            (Err("synthetic failure".into()), 1, 1, 0),
        );
    }
}
