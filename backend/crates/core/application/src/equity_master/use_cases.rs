use serde::Serialize;

use crate::equity_master_source::EquityMasterSource;
use crate::unit_of_work::SharedUnitOfWork;

use super::error::EquityMasterUseCaseError;
use super::repository::SharedEquityMasterRepository;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
pub struct EquityMasterSyncStats {
    pub stocks_upserted: usize,
}

#[derive(Clone)]
pub struct EquityMasterUseCases {
    unit_of_work: SharedUnitOfWork,
    repository: SharedEquityMasterRepository,
}

impl EquityMasterUseCases {
    pub fn new(unit_of_work: SharedUnitOfWork, repository: SharedEquityMasterRepository) -> Self {
        Self {
            unit_of_work,
            repository,
        }
    }

    pub async fn sync(
        &self,
        source: &dyn EquityMasterSource,
    ) -> Result<EquityMasterSyncStats, EquityMasterUseCaseError> {
        let entries = source.fetch_all_equities_master().await?;
        if entries.is_empty() {
            return Ok(EquityMasterSyncStats::default());
        }

        let transaction = self.unit_of_work.begin().await?;
        let stocks_upserted = self.repository.upsert(&transaction, &entries).await?;
        self.unit_of_work.commit(transaction).await?;
        Ok(EquityMasterSyncStats { stocks_upserted })
    }
}

#[cfg(all(test, feature = "test-support"))]
mod tests {
    use std::sync::Arc;

    use async_trait::async_trait;
    use core_domain::equity_master::{EquityMasterAttributeValue, EquityMasterEntry};
    use rstest::{fixture, rstest};
    use tokio::sync::Mutex;

    use crate::equity_master::repository::{
        EquityMasterRepository, EquityMasterRepositoryError, SharedEquityMasterRepository,
    };
    use crate::equity_master_source::{EquityMasterSource, EquityMasterSourceError};
    use crate::persistence::PersistenceError;
    use crate::unit_of_work::{
        FakeTransaction, FakeUnitOfWork, SharedUnitOfWork, UnitOfWorkTransaction,
    };

    use super::{EquityMasterSyncStats, EquityMasterUseCases};

    struct Harness {
        use_cases: EquityMasterUseCases,
        unit_of_work: Arc<FakeUnitOfWork>,
        repository: Arc<FakeRepository>,
    }

    #[fixture]
    fn harness() -> Harness {
        let unit_of_work = Arc::new(FakeUnitOfWork::new());
        let repository = Arc::new(FakeRepository::default());
        let unit_of_work_shared: SharedUnitOfWork = unit_of_work.clone();
        let repository_shared: SharedEquityMasterRepository = repository.clone();
        let use_cases = EquityMasterUseCases::new(unit_of_work_shared, repository_shared);
        Harness {
            use_cases,
            unit_of_work,
            repository,
        }
    }

    #[derive(Default)]
    struct FakeSource {
        entries: Vec<EquityMasterEntry>,
        error: Option<String>,
        calls: Mutex<usize>,
    }

    #[async_trait]
    impl EquityMasterSource for FakeSource {
        async fn fetch_all_equities_master(
            &self,
        ) -> Result<Vec<EquityMasterEntry>, EquityMasterSourceError> {
            *self.calls.lock().await += 1;
            if let Some(error) = &self.error {
                return Err(EquityMasterSourceError::Failed(error.clone()));
            }
            Ok(self.entries.clone())
        }
    }

    #[derive(Default)]
    struct FakeRepository {
        saved: Mutex<Vec<EquityMasterEntry>>,
        transaction_id: Mutex<Option<uuid::Uuid>>,
        upsert_calls: Mutex<usize>,
        upsert_error: Mutex<Option<String>>,
    }

    #[async_trait]
    impl EquityMasterRepository for FakeRepository {
        async fn upsert(
            &self,
            transaction: &UnitOfWorkTransaction,
            entries: &[EquityMasterEntry],
        ) -> Result<usize, EquityMasterRepositoryError> {
            *self.upsert_calls.lock().await += 1;
            if let Some(error) = self.upsert_error.lock().await.clone() {
                return Err(EquityMasterRepositoryError::Database(
                    PersistenceError::Database(error),
                ));
            }
            let transaction_id = transaction
                .downcast_ref::<FakeTransaction>()
                .map(|transaction| transaction.id)
                .ok_or(EquityMasterRepositoryError::InvalidTransaction)?;
            *self.transaction_id.lock().await = Some(transaction_id);
            self.saved.lock().await.extend_from_slice(entries);
            Ok(entries.len())
        }
    }

    fn equity_entry() -> EquityMasterEntry {
        EquityMasterEntry {
            id: "ZZ99".into(),
            name: "架空銘柄".into(),
            market: Some("架空市場".into()),
            tse_sector33: Some(EquityMasterAttributeValue {
                code: Some("1234".into()),
                name: "架空業種".into(),
            }),
            product_category: Some("000".into()),
        }
    }

    #[rstest]
    #[tokio::test]
    async fn sync_fetches_entries_and_saves_them_in_one_transaction(harness: Harness) {
        let source = FakeSource {
            entries: vec![equity_entry()],
            ..FakeSource::default()
        };

        let stats = harness
            .use_cases
            .sync(&source)
            .await
            .expect("sync succeeds");

        let begun = harness.unit_of_work.begun.lock().await.clone();
        let committed = harness.unit_of_work.committed.lock().await.clone();
        let transaction_id = *harness.repository.transaction_id.lock().await;
        let calls = *source.calls.lock().await;
        let saved = harness.repository.saved.lock().await.clone();
        assert_eq!(
            (
                stats,
                calls,
                saved,
                begun.len(),
                committed.len(),
                begun == committed && transaction_id == begun.first().copied(),
            ),
            (
                EquityMasterSyncStats { stocks_upserted: 1 },
                1,
                source.entries,
                1,
                1,
                true,
            ),
        );
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
        let saved = harness.repository.saved.lock().await.clone();
        let begun = harness.unit_of_work.begun.lock().await.len();
        let committed = harness.unit_of_work.committed.lock().await.len();
        assert_eq!(
            (stats, calls, saved, begun, committed),
            (EquityMasterSyncStats::default(), 1, vec![], 0, 0),
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
                Err("equity master source error: synthetic failure".into()),
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
            entries: vec![equity_entry()],
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
        let saved = harness.repository.saved.lock().await.clone();

        assert_eq!(
            (result, upsert_calls, begun, committed, saved),
            (Err("synthetic failure".into()), 1, 1, 0, vec![],),
        );
    }
}
