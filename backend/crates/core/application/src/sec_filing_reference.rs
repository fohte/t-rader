use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use chrono::{NaiveDate, Utc};
use core_domain::calendar_event::{CalendarEventReference, CalendarEventReferenceKind};
use serde::Serialize;
use thiserror::Error;

use crate::{
    calendar::{
        error::CalendarEventReferenceUseCaseError, references::CalendarEventReferenceUseCases,
        repository::CalendarEventRepositoryError,
    },
    sec_filing_source::SecFilingSource,
};

const SEC_SOURCE: &str = "sec";
const SEC_SUBMISSION_EVIDENCE: &str = "SEC submissions record";
const MAX_RUN_DURATION: Duration = Duration::from_secs(5 * 60);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecFilingReferenceTarget {
    pub event_source: String,
    pub event_external_id: String,
    pub stock_id: String,
    pub cik: String,
    pub fiscal_period: String,
}

#[async_trait::async_trait]
pub trait SecFilingTargetRepository: Send + Sync {
    async fn list_pending_sec_filing_targets(
        &self,
        today: NaiveDate,
    ) -> Result<Vec<SecFilingReferenceTarget>, CalendarEventRepositoryError>;
}

pub type SharedSecFilingTargetRepository = std::sync::Arc<dyn SecFilingTargetRepository>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
pub struct SecFilingReferenceStats {
    pub events_checked: usize,
    pub companies_checked: usize,
    pub external_failures: usize,
    pub references_upserted: usize,
    pub budget_exhausted: bool,
}

#[derive(Debug, Error)]
pub enum SecFilingReferenceUseCaseError {
    #[error(transparent)]
    Repository(#[from] CalendarEventRepositoryError),
    #[error(transparent)]
    Reference(#[from] CalendarEventReferenceUseCaseError),
}

#[derive(Clone)]
pub struct SecFilingReferenceUseCases {
    targets: SharedSecFilingTargetRepository,
    references: CalendarEventReferenceUseCases,
}

impl SecFilingReferenceUseCases {
    pub fn new(
        targets: SharedSecFilingTargetRepository,
        references: CalendarEventReferenceUseCases,
    ) -> Self {
        Self {
            targets,
            references,
        }
    }

    pub async fn fetch_pending(
        &self,
        source: &dyn SecFilingSource,
        today: NaiveDate,
    ) -> Result<SecFilingReferenceStats, SecFilingReferenceUseCaseError> {
        let started_at = Instant::now();
        let targets = self.targets.list_pending_sec_filing_targets(today).await?;
        let mut stats = SecFilingReferenceStats {
            ..SecFilingReferenceStats::default()
        };
        let mut targets_by_company: HashMap<(String, String), Vec<SecFilingReferenceTarget>> =
            HashMap::new();
        for target in targets {
            targets_by_company
                .entry((target.stock_id.clone(), target.cik.clone()))
                .or_default()
                .push(target);
        }

        let mut references = Vec::new();
        let mut seen_references = HashSet::new();
        for ((stock_id, cik), company_targets) in targets_by_company {
            let remaining = MAX_RUN_DURATION.saturating_sub(started_at.elapsed());
            if remaining.is_zero() {
                stats.budget_exhausted = true;
                break;
            }
            let Some(ticker) = stock_id.strip_prefix("US:") else {
                continue;
            };
            let report_dates = company_targets
                .iter()
                .filter_map(|target| parse_report_date(&target.fiscal_period))
                .collect::<HashSet<_>>();
            if report_dates.is_empty() {
                continue;
            }
            let mut report_dates = report_dates.into_iter().collect::<Vec<_>>();
            report_dates.sort_unstable();
            stats.events_checked += company_targets
                .iter()
                .filter(|target| parse_report_date(&target.fiscal_period).is_some())
                .count();
            stats.companies_checked += 1;

            let filings = match tokio::time::timeout(
                remaining,
                source.fetch_periodic_reports(&cik, ticker, &report_dates),
            )
            .await
            {
                Ok(Ok(filings)) => filings,
                Ok(Err(_)) => {
                    stats.external_failures += 1;
                    continue;
                }
                Err(_) => {
                    stats.external_failures += 1;
                    stats.budget_exhausted = true;
                    break;
                }
            };
            for filing in filings {
                if !is_supported_form(&filing.form) || !report_dates.contains(&filing.report_date) {
                    continue;
                }
                for target in company_targets.iter().filter(|target| {
                    parse_report_date(&target.fiscal_period) == Some(filing.report_date)
                }) {
                    let key = (
                        target.event_source.clone(),
                        target.event_external_id.clone(),
                        filing.accession_number.clone(),
                    );
                    if !seen_references.insert(key) {
                        continue;
                    }
                    references.push(CalendarEventReference {
                        event_source: target.event_source.clone(),
                        event_external_id: target.event_external_id.clone(),
                        kind: CalendarEventReferenceKind::IndividualDocument,
                        source: SEC_SOURCE.to_owned(),
                        external_id: filing.accession_number.clone(),
                        title: format!("SEC Form {}", filing.form),
                        url: Some(filing.url.clone()),
                        target_period_start: None,
                        target_period_end: Some(filing.report_date),
                        target_period_type: Some(filing.form.clone()),
                        publication_evidence: Some(SEC_SUBMISSION_EVIDENCE.to_owned()),
                        publication_evidence_url: None,
                        checked_at: Utc::now(),
                    });
                }
            }
        }

        stats.references_upserted = self.references.upsert(references).await?;
        Ok(stats)
    }
}

fn parse_report_date(value: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(value, "%Y-%m-%d").ok()
}

fn is_supported_form(form: &str) -> bool {
    matches!(form, "10-Q" | "10-K")
}

#[cfg(all(test, feature = "test-support"))]
mod tests {
    use std::{collections::VecDeque, sync::Arc};

    use async_trait::async_trait;
    use chrono::{DateTime, NaiveDate};
    use core_domain::calendar_event::{
        CalendarEvent, CalendarEventReadModel, CalendarEventReference,
    };
    use rstest::rstest;
    use tokio::sync::Mutex;

    use crate::{
        calendar::{
            references::CalendarEventReferenceUseCases,
            repository::{
                CalendarEventRepository, CalendarEventRepositoryError,
                SharedCalendarEventRepository,
            },
        },
        daily_bar_source::DateRange,
        sec_filing_source::{SecFilingDocument, SecFilingSource, SecFilingSourceError},
        unit_of_work::{FakeUnitOfWork, SharedUnitOfWork, UnitOfWorkTransaction},
    };

    use super::{
        SecFilingReferenceStats, SecFilingReferenceTarget, SecFilingReferenceUseCases,
        SecFilingTargetRepository,
    };

    #[derive(Debug, PartialEq, Eq)]
    struct NormalizedSecFilingReference {
        event_source: String,
        event_external_id: String,
        source: String,
        external_id: String,
        title: String,
        url: Option<String>,
        target_period_end: Option<NaiveDate>,
        target_period_type: Option<String>,
        publication_evidence: Option<String>,
        checked_at: DateTime<chrono::Utc>,
    }

    struct FakeCalendarEventRepository {
        upserted: Mutex<Vec<CalendarEventReference>>,
    }

    impl FakeCalendarEventRepository {
        fn new() -> Self {
            Self {
                upserted: Mutex::new(Vec::new()),
            }
        }
    }

    #[async_trait]
    impl CalendarEventRepository for FakeCalendarEventRepository {
        async fn list_events(
            &self,
            _date_range: &DateRange,
        ) -> Result<Vec<CalendarEvent>, CalendarEventRepositoryError> {
            Ok(Vec::new())
        }

        async fn enrich_events(
            &self,
            events: Vec<CalendarEvent>,
        ) -> Result<Vec<CalendarEventReadModel>, CalendarEventRepositoryError> {
            Ok(events.into_iter().map(Into::into).collect())
        }

        async fn list_references_for_event(
            &self,
            _event_source: &str,
            _event_external_id: &str,
        ) -> Result<Vec<CalendarEventReference>, CalendarEventRepositoryError> {
            Ok(Vec::new())
        }

        async fn find_next_earnings_for_stock(
            &self,
            _stock_id: &str,
            _from: NaiveDate,
        ) -> Result<Option<CalendarEvent>, CalendarEventRepositoryError> {
            Ok(None)
        }

        async fn upsert(
            &self,
            _transaction: &UnitOfWorkTransaction,
            events: Vec<CalendarEvent>,
        ) -> Result<usize, CalendarEventRepositoryError> {
            Ok(events.len())
        }

        async fn upsert_references(
            &self,
            _transaction: &UnitOfWorkTransaction,
            references: Vec<CalendarEventReference>,
        ) -> Result<usize, CalendarEventRepositoryError> {
            let count = references.len();
            self.upserted.lock().await.extend(references);
            Ok(count)
        }

        async fn delete_missing_future_events(
            &self,
            _transaction: &UnitOfWorkTransaction,
            _source: &str,
            _date_range: &DateRange,
            _today: NaiveDate,
            _external_ids: Vec<String>,
        ) -> Result<u64, CalendarEventRepositoryError> {
            Ok(0)
        }
    }

    struct FixedTargets(Vec<SecFilingReferenceTarget>);

    #[async_trait]
    impl SecFilingTargetRepository for FixedTargets {
        async fn list_pending_sec_filing_targets(
            &self,
            _today: NaiveDate,
        ) -> Result<Vec<SecFilingReferenceTarget>, CalendarEventRepositoryError> {
            Ok(self.0.clone())
        }
    }

    struct SequentialSource(Mutex<VecDeque<Result<Vec<SecFilingDocument>, SecFilingSourceError>>>);

    #[async_trait]
    impl SecFilingSource for SequentialSource {
        async fn fetch_periodic_reports(
            &self,
            _cik: &str,
            _ticker: &str,
            _report_dates: &[NaiveDate],
        ) -> Result<Vec<SecFilingDocument>, SecFilingSourceError> {
            self.0.lock().await.pop_front().unwrap_or(Ok(Vec::new()))
        }
    }

    fn target() -> SecFilingReferenceTarget {
        SecFilingReferenceTarget {
            event_source: "alpha_vantage".to_owned(),
            event_external_id: "US:QZ-A:2099-06-30".to_owned(),
            stock_id: "US:QZ-A".to_owned(),
            cik: "900000001".to_owned(),
            fiscal_period: "2099-06-30".to_owned(),
        }
    }

    fn filing(form: &str, report_date: NaiveDate) -> SecFilingDocument {
        SecFilingDocument {
            form: form.to_owned(),
            report_date,
            accession_number: "0900000001-99-000001".to_owned(),
            primary_document: "report.htm".to_owned(),
            url: "https://example.invalid/Archives/edgar/data/900000001/090000000199000001/report.htm"
                .to_owned(),
        }
    }

    fn harness(
        targets: Vec<SecFilingReferenceTarget>,
    ) -> (SecFilingReferenceUseCases, Arc<FakeCalendarEventRepository>) {
        let repository = Arc::new(FakeCalendarEventRepository::new());
        let repository_shared: SharedCalendarEventRepository = repository.clone();
        let unit_of_work: SharedUnitOfWork = Arc::new(FakeUnitOfWork::new());
        let references = CalendarEventReferenceUseCases::new(repository_shared, unit_of_work);
        let target_repository: Arc<dyn SecFilingTargetRepository> = Arc::new(FixedTargets(targets));
        (
            SecFilingReferenceUseCases::new(target_repository, references),
            repository,
        )
    }

    #[rstest]
    #[tokio::test]
    async fn stores_only_exact_periodic_reports_for_the_event_period() {
        let (use_cases, repository) = harness(vec![target()]);
        let source = SequentialSource(Mutex::new(VecDeque::from([Ok(vec![
            filing("10-Q", date(2099, 6, 30)),
            filing("10-K", date(2099, 6, 29)),
            filing("8-K", date(2099, 6, 30)),
        ])])));

        let stats = use_cases
            .fetch_pending(&source, date(2099, 7, 1))
            .await
            .expect("fetch succeeds");
        let stored = repository.upserted.lock().await.clone();
        let actual = (
            stats,
            stored
                .into_iter()
                .map(|reference| NormalizedSecFilingReference {
                    event_source: reference.event_source,
                    event_external_id: reference.event_external_id,
                    source: reference.source,
                    external_id: reference.external_id,
                    title: reference.title,
                    url: reference.url,
                    target_period_end: reference.target_period_end,
                    target_period_type: reference.target_period_type,
                    publication_evidence: reference.publication_evidence,
                    checked_at: reference.checked_at,
                })
                .collect::<Vec<_>>(),
        );

        let expected = (
            SecFilingReferenceStats {
                events_checked: 1,
                companies_checked: 1,
                external_failures: 0,
                references_upserted: 1,
                budget_exhausted: false,
            },
            vec![NormalizedSecFilingReference {
                event_source: "alpha_vantage".to_owned(),
                event_external_id: "US:QZ-A:2099-06-30".to_owned(),
                source: "sec".to_owned(),
                external_id: "0900000001-99-000001".to_owned(),
                title: "SEC Form 10-Q".to_owned(),
                url: Some("https://example.invalid/Archives/edgar/data/900000001/090000000199000001/report.htm".to_owned()),
                target_period_end: Some(date(2099, 6, 30)),
                target_period_type: Some("10-Q".to_owned()),
                publication_evidence: Some("SEC submissions record".to_owned()),
                checked_at: DateTime::<chrono::Utc>::UNIX_EPOCH,
            }],
        );
        assert_eq!(normalize_checked_at(actual), expected);
    }

    #[rstest]
    #[tokio::test]
    async fn external_failures_remain_pending_for_the_next_job() {
        let (use_cases, repository) = harness(vec![target()]);
        let source = SequentialSource(Mutex::new(VecDeque::from([
            Err(SecFilingSourceError::Failed("temporary failure".to_owned())),
            Ok(vec![filing("10-K", date(2099, 6, 30))]),
        ])));

        let first = use_cases
            .fetch_pending(&source, date(2099, 7, 1))
            .await
            .expect("first fetch records a retryable failure");
        let second = use_cases
            .fetch_pending(&source, date(2099, 7, 2))
            .await
            .expect("next fetch retries the event");
        let stored = repository.upserted.lock().await.clone();

        assert_eq!(
            (
                first,
                second,
                stored
                    .into_iter()
                    .map(|reference| (reference.title, reference.target_period_type))
                    .collect::<Vec<_>>(),
            ),
            (
                SecFilingReferenceStats {
                    events_checked: 1,
                    companies_checked: 1,
                    external_failures: 1,
                    references_upserted: 0,
                    budget_exhausted: false,
                },
                SecFilingReferenceStats {
                    events_checked: 1,
                    companies_checked: 1,
                    external_failures: 0,
                    references_upserted: 1,
                    budget_exhausted: false,
                },
                vec![("SEC Form 10-K".to_owned(), Some("10-K".to_owned()))],
            ),
        );
    }

    fn date(year: i32, month: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(year, month, day).unwrap_or(NaiveDate::MIN)
    }

    fn normalize_checked_at(
        actual: (SecFilingReferenceStats, Vec<NormalizedSecFilingReference>),
    ) -> (SecFilingReferenceStats, Vec<NormalizedSecFilingReference>) {
        let (stats, references) = actual;
        (
            stats,
            references
                .into_iter()
                .map(|mut reference| {
                    reference.checked_at = DateTime::<chrono::Utc>::UNIX_EPOCH;
                    reference
                })
                .collect(),
        )
    }
}
