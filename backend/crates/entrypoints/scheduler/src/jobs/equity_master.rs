use core_application::{
    equity_master::{EquityMasterSyncStats, EquityMasterUseCaseError, EquityMasterUseCases},
    equity_master_source::EquityMasterSource,
};
use graphile_worker::{IntoTaskHandlerResult, TaskHandler, WorkerContext};
use serde::{Deserialize, Serialize};

use super::{DAILY_TIMEOUT, require_source, run_with_state};

#[derive(Debug, Deserialize, Serialize)]
pub struct EquityMasterIngest;

impl TaskHandler for EquityMasterIngest {
    const IDENTIFIER: &'static str = "equity_master_ingest";

    async fn run(self, context: WorkerContext) -> impl IntoTaskHandlerResult {
        run_with_state(
            context,
            Self::IDENTIFIER,
            DAILY_TIMEOUT,
            |state| async move {
                let source = require_source(
                    state.dependencies.equity_master_source,
                    "J-Quants equity master source",
                )?;
                sync_equity_master(&state.dependencies.equity_master, source.as_ref()).await
            },
        )
        .await
    }
}

async fn sync_equity_master(
    use_cases: &EquityMasterUseCases,
    source: &dyn EquityMasterSource,
) -> Result<EquityMasterSyncStats, String> {
    let stats = use_cases
        .sync(source)
        .await
        .map_err(|error: EquityMasterUseCaseError| error.to_string())?;
    tracing::debug!(
        stocks_upserted = stats.stocks_upserted,
        "stock master sync cycle completed"
    );
    Ok(stats)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use async_trait::async_trait;
    use core_application::{
        equity_master::{
            EquityMasterRepository, EquityMasterRepositoryError, EquityMasterSyncStats,
            EquityMasterUseCases, SharedEquityMasterRepository,
        },
        equity_master_source::{EquityMasterSource, EquityMasterSourceError},
        unit_of_work::{FakeUnitOfWork, SharedUnitOfWork, UnitOfWorkTransaction},
    };
    use core_domain::equity_master::EquityMasterEntry;
    use tokio::sync::Mutex;

    use super::sync_equity_master;

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
    }

    #[async_trait]
    impl EquityMasterRepository for FakeRepository {
        async fn upsert(
            &self,
            _transaction: &UnitOfWorkTransaction,
            entries: &[EquityMasterEntry],
        ) -> Result<usize, EquityMasterRepositoryError> {
            self.saved.lock().await.extend_from_slice(entries);
            Ok(entries.len())
        }
    }

    #[tokio::test]
    async fn syncs_master_through_the_application_use_case() {
        let source = FakeSource {
            entries: vec![EquityMasterEntry {
                id: "ZZ99".to_string(),
                name: "架空銘柄".to_string(),
                market: Some("架空市場".to_string()),
                sector_name: Some("架空業種".to_string()),
                product_category: Some("000".to_string()),
            }],
            ..FakeSource::default()
        };
        let repository = Arc::new(FakeRepository::default());
        let repository_shared: SharedEquityMasterRepository = repository.clone();
        let unit_of_work = Arc::new(FakeUnitOfWork::new());
        let unit_of_work_shared: SharedUnitOfWork = unit_of_work.clone();
        let use_cases = EquityMasterUseCases::new(unit_of_work_shared, repository_shared);

        let stats = sync_equity_master(&use_cases, &source)
            .await
            .expect("sync succeeds");

        let begun = unit_of_work.begun.lock().await.clone();
        let committed = unit_of_work.committed.lock().await.clone();
        assert_eq!(
            (
                stats,
                *source.calls.lock().await,
                repository.saved.lock().await.clone(),
                begun.len(),
                committed.len(),
                begun == committed,
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
}
