use crate::equity_master_source::EquityMasterSource;
use crate::unit_of_work::SharedUnitOfWork;

use super::error::EquityMasterUseCaseError;
use super::repository::SharedEquityMasterRepository;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
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
    use core_domain::equity_master::EquityMasterEntry;
    use tokio::sync::Mutex;

    use crate::equity_master::repository::{
        EquityMasterRepository, EquityMasterRepositoryError, SharedEquityMasterRepository,
    };
    use crate::equity_master_source::{EquityMasterSource, EquityMasterSourceError};
    use crate::unit_of_work::{
        FakeTransaction, FakeUnitOfWork, SharedUnitOfWork, UnitOfWorkTransaction,
    };

    use super::{EquityMasterSyncStats, EquityMasterUseCases};

    #[derive(Default)]
    struct FakeSource {
        entries: Vec<EquityMasterEntry>,
        calls: Mutex<usize>,
    }

    #[async_trait]
    impl EquityMasterSource for FakeSource {
        async fn fetch_all_equities_master(
            &self,
        ) -> Result<Vec<EquityMasterEntry>, EquityMasterSourceError> {
            *self.calls.lock().await += 1;
            Ok(self.entries.clone())
        }
    }

    #[derive(Default)]
    struct FakeRepository {
        saved: Mutex<Vec<EquityMasterEntry>>,
        transaction_id: Mutex<Option<uuid::Uuid>>,
    }

    #[async_trait]
    impl EquityMasterRepository for FakeRepository {
        async fn upsert(
            &self,
            transaction: &UnitOfWorkTransaction,
            entries: &[EquityMasterEntry],
        ) -> Result<usize, EquityMasterRepositoryError> {
            let transaction_id = transaction
                .downcast_ref::<FakeTransaction>()
                .map(|transaction| transaction.id)
                .ok_or(EquityMasterRepositoryError::InvalidTransaction)?;
            *self.transaction_id.lock().await = Some(transaction_id);
            self.saved.lock().await.extend_from_slice(entries);
            Ok(entries.len())
        }
    }

    fn use_cases(
        unit_of_work: Arc<FakeUnitOfWork>,
        repository: Arc<FakeRepository>,
    ) -> EquityMasterUseCases {
        let unit_of_work: SharedUnitOfWork = unit_of_work;
        let repository: SharedEquityMasterRepository = repository;
        EquityMasterUseCases::new(unit_of_work, repository)
    }

    #[tokio::test]
    async fn sync_fetches_entries_and_saves_them_in_one_transaction() {
        let source = FakeSource {
            entries: vec![EquityMasterEntry {
                id: "ZZ99".into(),
                name: "架空銘柄".into(),
                market: Some("架空市場".into()),
                sector_name: Some("架空業種".into()),
                product_category: Some("000".into()),
            }],
            ..FakeSource::default()
        };
        let unit_of_work = Arc::new(FakeUnitOfWork::new());
        let repository = Arc::new(FakeRepository::default());

        let stats = use_cases(unit_of_work.clone(), repository.clone())
            .sync(&source)
            .await
            .expect("sync succeeds");

        let begun = unit_of_work.begun.lock().await.clone();
        let committed = unit_of_work.committed.lock().await.clone();
        let transaction_id = *repository.transaction_id.lock().await;
        let calls = *source.calls.lock().await;
        let saved = repository.saved.lock().await.clone();
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

    #[tokio::test]
    async fn sync_skips_the_transaction_when_the_source_is_empty() {
        let source = FakeSource::default();
        let unit_of_work = Arc::new(FakeUnitOfWork::new());
        let repository = Arc::new(FakeRepository::default());

        let stats = use_cases(unit_of_work.clone(), repository.clone())
            .sync(&source)
            .await
            .expect("sync succeeds");

        let calls = *source.calls.lock().await;
        let saved = repository.saved.lock().await.clone();
        let begun = unit_of_work.begun.lock().await.len();
        let committed = unit_of_work.committed.lock().await.len();
        assert_eq!(
            (stats, calls, saved, begun, committed),
            (EquityMasterSyncStats::default(), 1, vec![], 0, 0),
        );
    }
}
