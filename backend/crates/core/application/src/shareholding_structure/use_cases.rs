use async_trait::async_trait;
use chrono::{Duration, NaiveDate};
use core_domain::holdings::{
    CrossShareholdingDocument, LargeVolumeShareholdingDocument, MajorShareholderDocument,
};

use crate::daily_bar_source::DateRange;
use crate::shareholding_structure::repository::SharedShareholdingStructureRepository;
use crate::shareholding_structure_source::{
    ShareholdingStructureSource, ShareholdingStructureSourceError,
};
use crate::strategy_scope::StrategyScope;
use crate::unit_of_work::{SharedUnitOfWork, UnitOfWorkTransaction};

use super::error::ShareholdingStructureUseCaseError;
use super::types::ShareholdingStructureBySymbol;

const REFETCH_WINDOW_DAYS: i64 = 30;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ShareholdingStructureIngestStats {
    pub days_processed: usize,
    pub documents_saved: usize,
    pub failed_dates: usize,
}

#[derive(Clone)]
pub struct ShareholdingStructureUseCases {
    unit_of_work: SharedUnitOfWork,
    repository: SharedShareholdingStructureRepository,
}

impl ShareholdingStructureUseCases {
    pub fn new(
        unit_of_work: SharedUnitOfWork,
        repository: SharedShareholdingStructureRepository,
    ) -> Self {
        Self {
            unit_of_work,
            repository,
        }
    }

    pub async fn run_ingest_cycle(
        &self,
        source: &dyn ShareholdingStructureSource,
        today: NaiveDate,
    ) -> Result<ShareholdingStructureIngestStats, ShareholdingStructureUseCaseError> {
        let Some(fetchable_range) = source.fetchable_range(today) else {
            tracing::info!("取得元が取得可能範囲を公開しないため取り込みをスキップします");
            return Ok(ShareholdingStructureIngestStats::default());
        };

        let large_volume = self
            .run_endpoint::<LargeVolumeEndpoint>(source, &fetchable_range)
            .await;
        let cross_shareholding = self
            .run_endpoint::<CrossShareholdingEndpoint>(source, &fetchable_range)
            .await;
        let major_shareholder = self
            .run_endpoint::<MajorShareholderEndpoint>(source, &fetchable_range)
            .await;

        let mut stats = ShareholdingStructureIngestStats::default();
        let mut first_error = None;
        for (endpoint, result) in [
            (LargeVolumeEndpoint::NAME, large_volume),
            (CrossShareholdingEndpoint::NAME, cross_shareholding),
            (MajorShareholderEndpoint::NAME, major_shareholder),
        ] {
            match result {
                Ok(endpoint_stats) => {
                    tracing::debug!(
                        endpoint,
                        ?endpoint_stats,
                        "保有構造の取り込みが完了しました"
                    );
                    stats.days_processed += endpoint_stats.days_processed;
                    stats.documents_saved += endpoint_stats.documents_saved;
                    stats.failed_dates += endpoint_stats.failed_dates;
                }
                Err(error) => {
                    tracing::warn!(endpoint, %error, "保有構造の取り込みに失敗しました");
                    if first_error.is_none() {
                        first_error = Some(error);
                    }
                }
            }
        }

        first_error.map_or(Ok(stats), Err)
    }

    pub async fn find_for_symbol(
        &self,
        _scope: StrategyScope,
        symbol: &str,
        limit: u64,
    ) -> Result<ShareholdingStructureBySymbol, ShareholdingStructureUseCaseError> {
        self.repository
            .find_for_symbol(symbol, limit)
            .await
            .map_err(Into::into)
    }

    async fn run_endpoint<E: ShareholdingEndpoint>(
        &self,
        source: &dyn ShareholdingStructureSource,
        fetchable_range: &DateRange,
    ) -> Result<ShareholdingStructureIngestStats, ShareholdingStructureUseCaseError> {
        let latest_submitted_on = E::latest_submitted_on(self.repository.as_ref()).await?;
        let from = ingest_start_date(
            fetchable_range.from,
            E::available_from(),
            latest_submitted_on,
        );
        if from > fetchable_range.to {
            return Ok(ShareholdingStructureIngestStats::default());
        }

        let mut stats = ShareholdingStructureIngestStats::default();
        let mut retry_dates = Vec::new();
        let mut date = from;
        while date <= fetchable_range.to {
            match self.ingest_date::<E>(source, date).await {
                Ok(count) => stats.documents_saved += count,
                Err(IngestDateError::Source(error)) => {
                    tracing::warn!(endpoint = E::NAME, %date, %error, "書類の取得に失敗し、この cycle 終了後に再試行します");
                    retry_dates.push(date);
                }
                Err(IngestDateError::UseCase(error)) => return Err(error),
            }
            stats.days_processed += 1;
            date += Duration::days(1);
        }

        for date in retry_dates {
            match self.ingest_date::<E>(source, date).await {
                Ok(count) => stats.documents_saved += count,
                Err(IngestDateError::Source(error)) => {
                    stats.failed_dates += 1;
                    tracing::error!(
                        endpoint = E::NAME,
                        %date,
                        %error,
                        "書類の再取得にも失敗しました。この日付の書類は取り込めていません"
                    );
                }
                Err(IngestDateError::UseCase(error)) => return Err(error),
            }
        }

        Ok(stats)
    }

    async fn ingest_date<E: ShareholdingEndpoint>(
        &self,
        source: &dyn ShareholdingStructureSource,
        date: NaiveDate,
    ) -> Result<usize, IngestDateError> {
        let documents = E::fetch(source, date)
            .await
            .map_err(IngestDateError::Source)?;
        if documents.is_empty() {
            return Ok(0);
        }

        let transaction = self
            .unit_of_work
            .begin()
            .await
            .map_err(ShareholdingStructureUseCaseError::from)
            .map_err(IngestDateError::UseCase)?;
        let count = E::upsert(self.repository.as_ref(), &transaction, documents)
            .await
            .map_err(ShareholdingStructureUseCaseError::from)
            .map_err(IngestDateError::UseCase)?;
        self.unit_of_work
            .commit(transaction)
            .await
            .map_err(ShareholdingStructureUseCaseError::from)
            .map_err(IngestDateError::UseCase)?;
        Ok(count)
    }
}

fn ingest_start_date(
    fetchable_from: NaiveDate,
    available_from: NaiveDate,
    latest_submitted_on: Option<NaiveDate>,
) -> NaiveDate {
    let earliest = fetchable_from.max(available_from);
    latest_submitted_on
        .map(|latest| (latest - Duration::days(REFETCH_WINDOW_DAYS)).max(earliest))
        .unwrap_or(earliest)
}

enum IngestDateError {
    Source(ShareholdingStructureSourceError),
    UseCase(ShareholdingStructureUseCaseError),
}

#[async_trait]
trait ShareholdingEndpoint: Send + Sync {
    type Document: Send;

    const NAME: &'static str;

    fn available_from() -> NaiveDate;

    async fn latest_submitted_on(
        repository: &dyn super::repository::ShareholdingStructureRepository,
    ) -> Result<Option<NaiveDate>, super::ShareholdingStructureRepositoryError>;

    async fn fetch(
        source: &dyn ShareholdingStructureSource,
        date: NaiveDate,
    ) -> Result<Vec<Self::Document>, ShareholdingStructureSourceError>;

    async fn upsert(
        repository: &dyn super::repository::ShareholdingStructureRepository,
        transaction: &UnitOfWorkTransaction,
        documents: Vec<Self::Document>,
    ) -> Result<usize, super::ShareholdingStructureRepositoryError>;
}

struct LargeVolumeEndpoint;
struct MajorShareholderEndpoint;
struct CrossShareholdingEndpoint;

#[async_trait]
impl ShareholdingEndpoint for LargeVolumeEndpoint {
    type Document = LargeVolumeShareholdingDocument;

    const NAME: &'static str = "large_volume_shareholdings";

    fn available_from() -> NaiveDate {
        NaiveDate::from_ymd_opt(2021, 7, 1).unwrap_or_default()
    }

    async fn latest_submitted_on(
        repository: &dyn super::repository::ShareholdingStructureRepository,
    ) -> Result<Option<NaiveDate>, super::ShareholdingStructureRepositoryError> {
        repository.latest_large_volume_submitted_on().await
    }

    async fn fetch(
        source: &dyn ShareholdingStructureSource,
        date: NaiveDate,
    ) -> Result<Vec<Self::Document>, ShareholdingStructureSourceError> {
        source.fetch_large_volume_documents(date).await
    }

    async fn upsert(
        repository: &dyn super::repository::ShareholdingStructureRepository,
        transaction: &UnitOfWorkTransaction,
        documents: Vec<Self::Document>,
    ) -> Result<usize, super::ShareholdingStructureRepositoryError> {
        repository.upsert_large_volume(transaction, documents).await
    }
}

#[async_trait]
impl ShareholdingEndpoint for MajorShareholderEndpoint {
    type Document = MajorShareholderDocument;

    const NAME: &'static str = "major_shareholders";

    fn available_from() -> NaiveDate {
        NaiveDate::from_ymd_opt(2016, 6, 1).unwrap_or_default()
    }

    async fn latest_submitted_on(
        repository: &dyn super::repository::ShareholdingStructureRepository,
    ) -> Result<Option<NaiveDate>, super::ShareholdingStructureRepositoryError> {
        repository.latest_major_shareholder_submitted_on().await
    }

    async fn fetch(
        source: &dyn ShareholdingStructureSource,
        date: NaiveDate,
    ) -> Result<Vec<Self::Document>, ShareholdingStructureSourceError> {
        source.fetch_major_shareholder_documents(date).await
    }

    async fn upsert(
        repository: &dyn super::repository::ShareholdingStructureRepository,
        transaction: &UnitOfWorkTransaction,
        documents: Vec<Self::Document>,
    ) -> Result<usize, super::ShareholdingStructureRepositoryError> {
        repository
            .upsert_major_shareholders(transaction, documents)
            .await
    }
}

#[async_trait]
impl ShareholdingEndpoint for CrossShareholdingEndpoint {
    type Document = CrossShareholdingDocument;

    const NAME: &'static str = "cross_shareholdings";

    fn available_from() -> NaiveDate {
        NaiveDate::from_ymd_opt(2020, 3, 31).unwrap_or_default()
    }

    async fn latest_submitted_on(
        repository: &dyn super::repository::ShareholdingStructureRepository,
    ) -> Result<Option<NaiveDate>, super::ShareholdingStructureRepositoryError> {
        repository.latest_cross_shareholding_submitted_on().await
    }

    async fn fetch(
        source: &dyn ShareholdingStructureSource,
        date: NaiveDate,
    ) -> Result<Vec<Self::Document>, ShareholdingStructureSourceError> {
        source.fetch_cross_shareholding_documents(date).await
    }

    async fn upsert(
        repository: &dyn super::repository::ShareholdingStructureRepository,
        transaction: &UnitOfWorkTransaction,
        documents: Vec<Self::Document>,
    ) -> Result<usize, super::ShareholdingStructureRepositoryError> {
        repository
            .upsert_cross_shareholdings(transaction, documents)
            .await
    }
}

#[cfg(all(test, feature = "test-support"))]
mod tests {
    use std::collections::HashMap;
    use std::sync::Arc;

    use async_trait::async_trait;
    use chrono::NaiveDate;
    use core_domain::holdings::{
        CrossShareholdingDocument, LargeVolumeShareholdingDocument, MajorShareholderDocument,
        ShareholdingDocumentMetadata,
    };
    use rstest::rstest;

    use crate::daily_bar_source::DateRange;
    use crate::persistence::PersistenceError;
    use crate::shareholding_structure::{
        ShareholdingStructureBySymbol, ShareholdingStructureRepository,
        ShareholdingStructureRepositoryError, ShareholdingStructureUseCases,
    };
    use crate::shareholding_structure_source::{
        ShareholdingStructureSource, ShareholdingStructureSourceError,
    };
    use crate::unit_of_work::{
        FakeTransaction, FakeUnitOfWork, SharedUnitOfWork, UnitOfWorkTransaction,
    };
    use tokio::sync::Mutex;

    use super::ingest_start_date;

    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    enum EndpointKind {
        LargeVolume,
        MajorShareholder,
        CrossShareholding,
    }

    #[derive(Default)]
    struct FakeSource {
        range: Option<DateRange>,
        large_volume: HashMap<NaiveDate, Vec<LargeVolumeShareholdingDocument>>,
        failed_large_volume_date: Option<NaiveDate>,
        failed_large_volume_attempts: usize,
        large_volume_attempts: Mutex<HashMap<NaiveDate, usize>>,
        requests: Mutex<Vec<(EndpointKind, NaiveDate)>>,
    }

    #[async_trait]
    impl ShareholdingStructureSource for FakeSource {
        async fn fetch_large_volume_documents(
            &self,
            date: NaiveDate,
        ) -> Result<Vec<LargeVolumeShareholdingDocument>, ShareholdingStructureSourceError>
        {
            self.requests
                .lock()
                .await
                .push((EndpointKind::LargeVolume, date));
            let attempts = {
                let mut attempts = self.large_volume_attempts.lock().await;
                let attempts = attempts.entry(date).or_default();
                *attempts += 1;
                *attempts
            };
            if self.failed_large_volume_date == Some(date)
                && attempts <= self.failed_large_volume_attempts
            {
                return Err(ShareholdingStructureSourceError::Failed(
                    "synthetic failure".into(),
                ));
            }
            Ok(self.large_volume.get(&date).cloned().unwrap_or_default())
        }

        async fn fetch_major_shareholder_documents(
            &self,
            date: NaiveDate,
        ) -> Result<Vec<MajorShareholderDocument>, ShareholdingStructureSourceError> {
            self.requests
                .lock()
                .await
                .push((EndpointKind::MajorShareholder, date));
            Ok(Vec::new())
        }

        async fn fetch_cross_shareholding_documents(
            &self,
            date: NaiveDate,
        ) -> Result<Vec<CrossShareholdingDocument>, ShareholdingStructureSourceError> {
            self.requests
                .lock()
                .await
                .push((EndpointKind::CrossShareholding, date));
            Ok(Vec::new())
        }

        fn fetchable_range(&self, _today: NaiveDate) -> Option<DateRange> {
            self.range.clone()
        }
    }

    #[derive(Default)]
    struct FakeRepository {
        large_volume: Mutex<Vec<LargeVolumeShareholdingDocument>>,
        transaction_ids: Mutex<Vec<uuid::Uuid>>,
        fail_latest_large_volume: bool,
    }

    #[async_trait]
    impl ShareholdingStructureRepository for FakeRepository {
        async fn latest_large_volume_submitted_on(
            &self,
        ) -> Result<Option<NaiveDate>, ShareholdingStructureRepositoryError> {
            if self.fail_latest_large_volume {
                return Err(PersistenceError::Database("synthetic failure".into()).into());
            }
            Ok(None)
        }

        async fn upsert_large_volume(
            &self,
            transaction: &UnitOfWorkTransaction,
            documents: Vec<LargeVolumeShareholdingDocument>,
        ) -> Result<usize, ShareholdingStructureRepositoryError> {
            let transaction_id = transaction
                .downcast_ref::<FakeTransaction>()
                .map(|transaction| transaction.id)
                .ok_or(ShareholdingStructureRepositoryError::InvalidTransaction)?;
            self.transaction_ids.lock().await.push(transaction_id);
            let count = documents.len();
            self.large_volume.lock().await.extend(documents);
            Ok(count)
        }

        async fn latest_major_shareholder_submitted_on(
            &self,
        ) -> Result<Option<NaiveDate>, ShareholdingStructureRepositoryError> {
            Ok(None)
        }

        async fn upsert_major_shareholders(
            &self,
            _transaction: &UnitOfWorkTransaction,
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
            _transaction: &UnitOfWorkTransaction,
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
                large_volume_reports: vec![],
                major_shareholders: None,
                cross_shareholdings: None,
            })
        }
    }

    fn date(year: i32, month: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(year, month, day).unwrap_or_default()
    }

    fn large_volume_document(submitted_on: NaiveDate) -> LargeVolumeShareholdingDocument {
        LargeVolumeShareholdingDocument {
            metadata: ShareholdingDocumentMetadata {
                document_id: "SAMPLE-DOC".into(),
                stock_code: Some("ZZ990".into()),
                filer_code: "E99999".into(),
                submitted_on,
            },
            content: None,
        }
    }

    #[rstest]
    #[case::starts_at_fetchable_range(None, date(2021, 7, 1))]
    #[case::refetches_the_lookback(Some(date(2021, 8, 15)), date(2021, 7, 16))]
    #[case::clamps_to_source_range(Some(date(2021, 6, 1)), date(2021, 7, 1))]
    fn ingest_start_date_cases(
        #[case] latest_submitted_on: Option<NaiveDate>,
        #[case] expected: NaiveDate,
    ) {
        assert_eq!(
            ingest_start_date(date(2021, 7, 1), date(2021, 7, 1), latest_submitted_on,),
            expected,
        );
    }

    #[rstest]
    #[case::retry_recovers_after_one_failure(1, true, 0)]
    #[case::counts_date_after_both_attempts_fail(2, false, 1)]
    #[tokio::test]
    async fn ingest_retries_source_failures_and_reports_the_result(
        #[case] failed_attempts: usize,
        #[case] expected_document_saved: bool,
        #[case] expected_failed_dates: usize,
    ) {
        let from = date(2021, 7, 1);
        let retry_date = date(2021, 7, 2);
        let source = FakeSource {
            range: Some(DateRange {
                from,
                to: retry_date,
            }),
            large_volume: HashMap::from([(retry_date, vec![large_volume_document(retry_date)])]),
            failed_large_volume_date: Some(retry_date),
            failed_large_volume_attempts: failed_attempts,
            ..FakeSource::default()
        };
        let unit_of_work = Arc::new(FakeUnitOfWork::new());
        let repository = Arc::new(FakeRepository::default());
        let unit_of_work_shared: SharedUnitOfWork = unit_of_work.clone();
        let repository_shared = repository.clone();
        let use_cases = ShareholdingStructureUseCases::new(unit_of_work_shared, repository_shared);

        let stats = use_cases
            .run_ingest_cycle(&source, retry_date)
            .await
            .expect("ingest succeeds");

        let begun = unit_of_work.begun.lock().await.clone();
        let committed = unit_of_work.committed.lock().await.clone();
        let saved_transaction = repository.transaction_ids.lock().await.clone();
        let saved_documents = repository.large_volume.lock().await.clone();
        let request_count = source.requests.lock().await.len();
        let expected_saved_documents = expected_document_saved
            .then(|| large_volume_document(retry_date))
            .into_iter()
            .collect::<Vec<_>>();
        let expected_transaction_count = usize::from(expected_document_saved);
        assert_eq!(
            (
                stats,
                request_count,
                saved_documents,
                begun.len(),
                committed.len(),
                begun == committed && committed == saved_transaction,
            ),
            (
                super::ShareholdingStructureIngestStats {
                    days_processed: 6,
                    documents_saved: expected_transaction_count,
                    failed_dates: expected_failed_dates,
                },
                7,
                expected_saved_documents,
                expected_transaction_count,
                expected_transaction_count,
                true,
            ),
        );
    }

    #[tokio::test]
    async fn ingest_continues_other_endpoints_when_repository_lookup_fails() {
        let from = date(2021, 7, 1);
        let to = date(2021, 7, 2);
        let source = FakeSource {
            range: Some(DateRange { from, to }),
            ..FakeSource::default()
        };
        let unit_of_work = Arc::new(FakeUnitOfWork::new());
        let repository = Arc::new(FakeRepository {
            fail_latest_large_volume: true,
            ..FakeRepository::default()
        });
        let unit_of_work_shared: SharedUnitOfWork = unit_of_work.clone();
        let repository_shared = repository.clone();
        let use_cases = ShareholdingStructureUseCases::new(unit_of_work_shared, repository_shared);

        let result = use_cases
            .run_ingest_cycle(&source, to)
            .await
            .map_err(|error| error.to_string());
        let requests = source.requests.lock().await.clone();
        let begun = unit_of_work.begun.lock().await.clone();
        let committed = unit_of_work.committed.lock().await.clone();

        assert_eq!(
            (result, requests, begun, committed),
            (
                Err("synthetic failure".into()),
                vec![
                    (EndpointKind::CrossShareholding, from),
                    (EndpointKind::CrossShareholding, to),
                    (EndpointKind::MajorShareholder, from),
                    (EndpointKind::MajorShareholder, to),
                ],
                vec![],
                vec![],
            ),
        );
    }
}
