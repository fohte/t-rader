use core_application::{
    equity_master::{EquityMasterSyncStats, EquityMasterUseCaseError, EquityMasterUseCases},
    equity_master_source::EquityMasterSource,
};
use graphile_worker::{IntoTaskHandlerResult, TaskHandler, WorkerContext};
use serde::{Deserialize, Serialize};

use super::{DAILY_TIMEOUT, require_source, run_with_ingest_run_log_state};

#[derive(Debug, Deserialize, Serialize)]
pub struct EquityMasterIngest;

impl TaskHandler for EquityMasterIngest {
    const IDENTIFIER: &'static str = "equity_master_ingest";

    async fn run(self, context: WorkerContext) -> impl IntoTaskHandlerResult {
        run_with_ingest_run_log_state(
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
    use rstest::{fixture, rstest};

    use super::sync_equity_master;

    #[derive(Default)]
    struct FakeSource {
        entries: Vec<EquityMasterEntry>,
        error: Option<String>,
    }

    #[async_trait]
    impl EquityMasterSource for FakeSource {
        async fn fetch_all_equities_master(
            &self,
        ) -> Result<Vec<EquityMasterEntry>, EquityMasterSourceError> {
            if let Some(error) = &self.error {
                return Err(EquityMasterSourceError::Failed(error.clone()));
            }
            Ok(self.entries.clone())
        }
    }

    struct FakeRepository;

    #[async_trait]
    impl EquityMasterRepository for FakeRepository {
        async fn upsert(
            &self,
            _transaction: &UnitOfWorkTransaction,
            entries: &[EquityMasterEntry],
        ) -> Result<usize, EquityMasterRepositoryError> {
            Ok(entries.len())
        }
    }

    #[fixture]
    fn use_cases() -> EquityMasterUseCases {
        let repository: SharedEquityMasterRepository = Arc::new(FakeRepository);
        let unit_of_work: SharedUnitOfWork = Arc::new(FakeUnitOfWork::new());
        EquityMasterUseCases::new(unit_of_work, repository)
    }

    fn equity_entry() -> EquityMasterEntry {
        EquityMasterEntry {
            id: "ZZ99".to_string(),
            name: "架空銘柄".to_string(),
            market: Some("架空市場".to_string()),
            sector_name: Some("架空業種".to_string()),
            product_category: Some("000".to_string()),
        }
    }

    #[rstest]
    #[case::success(
        FakeSource { entries: vec![equity_entry()], error: None },
        Ok(EquityMasterSyncStats { stocks_upserted: 1 }),
    )]
    #[case::source_error(
        FakeSource { entries: vec![], error: Some("synthetic failure".to_string()) },
        Err("equity master source error: synthetic failure".to_string()),
    )]
    #[tokio::test]
    async fn returns_the_application_use_case_result(
        use_cases: EquityMasterUseCases,
        #[case] source: FakeSource,
        #[case] expected: Result<EquityMasterSyncStats, String>,
    ) {
        assert_eq!(sync_equity_master(&use_cases, &source).await, expected);
    }
}
