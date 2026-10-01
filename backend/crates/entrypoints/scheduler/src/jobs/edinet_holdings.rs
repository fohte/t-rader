use chrono::Utc;
use core_application::{
    shareholding_structure::{
        ShareholdingStructureIngestStats, ShareholdingStructureUseCaseError,
        ShareholdingStructureUseCases,
    },
    shareholding_structure_source::ShareholdingStructureSource,
};
use graphile_worker::{IntoTaskHandlerResult, TaskHandler, WorkerContext};
use serde::{Deserialize, Serialize};

use super::{DAILY_TIMEOUT, require_source, run_with_ingest_run_log_state};

#[derive(Debug, Deserialize, Serialize)]
pub struct ShareholdingStructureIngest;

impl TaskHandler for ShareholdingStructureIngest {
    const IDENTIFIER: &'static str = "shareholding_structure_ingest";

    async fn run(self, context: WorkerContext) -> impl IntoTaskHandlerResult {
        run_with_ingest_run_log_state(
            context,
            Self::IDENTIFIER,
            DAILY_TIMEOUT,
            |state| async move {
                let source = require_source(
                    state.dependencies.shareholding_structure_source,
                    "J-Quants shareholding structure source",
                )?;
                ingest_shareholding_structures(
                    &state.dependencies.shareholding_structures,
                    source.as_ref(),
                )
                .await
            },
        )
        .await
    }
}

async fn ingest_shareholding_structures(
    use_cases: &ShareholdingStructureUseCases,
    source: &dyn ShareholdingStructureSource,
) -> Result<ShareholdingStructureIngestStats, String> {
    let stats = use_cases
        .run_ingest_cycle(source, Utc::now().date_naive())
        .await
        .map_err(|error: ShareholdingStructureUseCaseError| error.to_string())?;
    tracing::debug!(
        days_processed = stats.days_processed,
        documents_saved = stats.documents_saved,
        failed_dates = stats.failed_dates,
        "shareholding structure ingest cycle completed"
    );
    Ok(stats)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use async_trait::async_trait;
    use chrono::NaiveDate;
    use core_application::{
        daily_bar_source::DateRange,
        shareholding_structure::{
            ShareholdingStructureBySymbol, ShareholdingStructureRepository,
            ShareholdingStructureRepositoryError, ShareholdingStructureUseCases,
        },
        shareholding_structure_source::{
            ShareholdingStructureSource, ShareholdingStructureSourceError,
        },
        unit_of_work::{FakeUnitOfWork, SharedUnitOfWork},
    };
    use core_domain::holdings::{
        CrossShareholdingDocument, LargeVolumeShareholdingDocument, MajorShareholderDocument,
    };

    use super::ingest_shareholding_structures;

    struct EmptySource;

    #[async_trait]
    impl ShareholdingStructureSource for EmptySource {
        async fn fetch_large_volume_documents(
            &self,
            _date: NaiveDate,
        ) -> Result<Vec<LargeVolumeShareholdingDocument>, ShareholdingStructureSourceError>
        {
            Ok(Vec::new())
        }

        async fn fetch_major_shareholder_documents(
            &self,
            _date: NaiveDate,
        ) -> Result<Vec<MajorShareholderDocument>, ShareholdingStructureSourceError> {
            Ok(Vec::new())
        }

        async fn fetch_cross_shareholding_documents(
            &self,
            _date: NaiveDate,
        ) -> Result<Vec<CrossShareholdingDocument>, ShareholdingStructureSourceError> {
            Ok(Vec::new())
        }

        fn fetchable_range(&self, _today: NaiveDate) -> Option<DateRange> {
            let date = NaiveDate::from_ymd_opt(2099, 1, 8)?;
            Some(DateRange {
                from: date,
                to: date,
            })
        }
    }

    struct EmptyRepository;

    #[async_trait]
    impl ShareholdingStructureRepository for EmptyRepository {
        async fn latest_large_volume_submitted_on(
            &self,
        ) -> Result<Option<NaiveDate>, ShareholdingStructureRepositoryError> {
            Ok(None)
        }

        async fn upsert_large_volume(
            &self,
            _transaction: &core_application::unit_of_work::UnitOfWorkTransaction,
            documents: Vec<LargeVolumeShareholdingDocument>,
        ) -> Result<usize, ShareholdingStructureRepositoryError> {
            Ok(documents.len())
        }

        async fn latest_major_shareholder_submitted_on(
            &self,
        ) -> Result<Option<NaiveDate>, ShareholdingStructureRepositoryError> {
            Ok(None)
        }

        async fn upsert_major_shareholders(
            &self,
            _transaction: &core_application::unit_of_work::UnitOfWorkTransaction,
            documents: Vec<MajorShareholderDocument>,
        ) -> Result<usize, ShareholdingStructureRepositoryError> {
            Ok(documents.len())
        }

        async fn latest_cross_shareholding_submitted_on(
            &self,
        ) -> Result<Option<NaiveDate>, ShareholdingStructureRepositoryError> {
            Ok(None)
        }

        async fn upsert_cross_shareholdings(
            &self,
            _transaction: &core_application::unit_of_work::UnitOfWorkTransaction,
            documents: Vec<CrossShareholdingDocument>,
        ) -> Result<usize, ShareholdingStructureRepositoryError> {
            Ok(documents.len())
        }

        async fn find_for_symbol(
            &self,
            _symbol: &str,
            _limit: u64,
        ) -> Result<ShareholdingStructureBySymbol, ShareholdingStructureRepositoryError> {
            Ok(ShareholdingStructureBySymbol {
                large_volume_reports: Vec::new(),
                major_shareholders: None,
                cross_shareholdings: None,
            })
        }
    }

    #[tokio::test]
    async fn runs_the_shareholding_structure_use_case() {
        let unit_of_work: SharedUnitOfWork = Arc::new(FakeUnitOfWork::new());
        let use_cases = ShareholdingStructureUseCases::new(unit_of_work, Arc::new(EmptyRepository));

        assert_eq!(
            ingest_shareholding_structures(&use_cases, &EmptySource).await,
            Ok(super::ShareholdingStructureIngestStats {
                days_processed: 3,
                documents_saved: 0,
                failed_dates: 0,
            }),
        );
    }
}
