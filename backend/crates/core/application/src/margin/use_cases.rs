use chrono::{Duration, NaiveDate};

use crate::margin_source::MarginSource;
use crate::strategy_scope::StrategyScope;

use super::error::MarginUseCaseError;
use super::repository::SharedMarginRepository;
use super::types::{IngestStats, MarginQuery, MarginReadResult};

const REFETCH_LOOKBACK_DAYS: i64 = 30;

#[derive(Clone)]
pub struct MarginUseCases {
    repository: SharedMarginRepository,
}

impl MarginUseCases {
    pub fn new(repository: SharedMarginRepository) -> Self {
        Self { repository }
    }

    pub async fn ingest(
        &self,
        source: &dyn MarginSource,
        today: NaiveDate,
    ) -> Result<(IngestStats, IngestStats), MarginUseCaseError> {
        let Some(range) = source.fetchable_range(today) else {
            tracing::debug!("信用残データを取得できないため、取り込みをスキップします");
            return Ok((IngestStats::default(), IngestStats::default()));
        };

        let interest_earliest = range.from.max(margin_interest_start_date());
        let interest_start = resolve_start_date(
            self.repository.find_latest_margin_interest_date().await?,
            interest_earliest,
        );
        let interest_stats = self.ingest_interest(source, interest_start, range.to).await;

        let alert_earliest = range.from.max(margin_alert_start_date());
        let alert_start = resolve_start_date(
            self.repository.find_latest_margin_alert_pub_date().await?,
            alert_earliest,
        );
        let alert_stats = self.ingest_alert(source, alert_start, range.to).await;

        Ok((interest_stats, alert_stats))
    }

    pub async fn read(
        &self,
        _scope: StrategyScope,
        query: MarginQuery,
    ) -> Result<MarginReadResult, MarginUseCaseError> {
        if let (Some(from), Some(to)) = (query.from, query.to)
            && from > to
        {
            return Err(MarginUseCaseError::InvalidDateRange);
        }
        self.repository.read(query).await.map_err(Into::into)
    }

    async fn ingest_interest(
        &self,
        source: &dyn MarginSource,
        start: NaiveDate,
        end: NaiveDate,
    ) -> IngestStats {
        let mut stats = IngestStats::default();
        let mut date = start;
        while date <= end {
            match source.fetch_margin_interest(date).await {
                Ok(records) => {
                    let count = records.len();
                    match self.repository.upsert_margin_interest(records).await {
                        Ok(()) => {
                            stats.days_fetched += 1;
                            stats.rows_upserted += count;
                        }
                        Err(error) => {
                            tracing::warn!(%date, error = %error, "信用取引週末残高の保存に失敗、次の日に進みます");
                        }
                    }
                }
                Err(error) => {
                    tracing::warn!(%date, error = %error, "信用取引週末残高の取得に失敗、次の日に進みます");
                }
            }
            date += Duration::days(1);
        }
        stats
    }

    async fn ingest_alert(
        &self,
        source: &dyn MarginSource,
        start: NaiveDate,
        end: NaiveDate,
    ) -> IngestStats {
        let mut stats = IngestStats::default();
        let mut date = start;
        while date <= end {
            match source.fetch_margin_alert(date).await {
                Ok(records) => {
                    let count = records.len();
                    match self.repository.upsert_margin_alert(records).await {
                        Ok(()) => {
                            stats.days_fetched += 1;
                            stats.rows_upserted += count;
                        }
                        Err(error) => {
                            tracing::warn!(%date, error = %error, "日々公表信用取引残高の保存に失敗、次の日に進みます");
                        }
                    }
                }
                Err(error) => {
                    tracing::warn!(%date, error = %error, "日々公表信用取引残高の取得に失敗、次の日に進みます");
                }
            }
            date += Duration::days(1);
        }
        stats
    }
}

fn margin_interest_start_date() -> NaiveDate {
    NaiveDate::from_ymd_opt(2012, 2, 10).unwrap_or_default()
}

fn margin_alert_start_date() -> NaiveDate {
    NaiveDate::from_ymd_opt(2008, 5, 8).unwrap_or_default()
}

fn resolve_start_date(latest_stored: Option<NaiveDate>, earliest: NaiveDate) -> NaiveDate {
    match latest_stored {
        None => earliest,
        Some(latest) => (latest - Duration::days(REFETCH_LOOKBACK_DAYS)).max(earliest),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use async_trait::async_trait;
    use chrono::NaiveDate;
    use core_domain::margin::{MarginAlertRecord, MarginInterestRecord, PubReason};
    use rstest::rstest;

    use crate::daily_bar_source::DateRange;
    use crate::margin::{
        FakeMarginRepository, IngestStats, MarginQuery, MarginReadResult, MarginRepository,
    };
    use crate::margin_source::{MarginSource, MarginSourceError};
    use crate::strategy_scope::{StrategyScope, StrategyScopeSource, StrategyScopeSourceError};
    use uuid::Uuid;

    use super::MarginUseCases;

    #[derive(Default)]
    struct FakeMarginSource {
        range: Option<DateRange>,
        interest_calls: Mutex<Vec<NaiveDate>>,
        alert_calls: Mutex<Vec<NaiveDate>>,
        interest_failure: Option<NaiveDate>,
        alert_failure: Option<NaiveDate>,
        interest_record: Option<MarginInterestRecord>,
        alert_record: Option<MarginAlertRecord>,
    }

    #[async_trait]
    impl MarginSource for FakeMarginSource {
        async fn fetch_margin_interest(
            &self,
            date: NaiveDate,
        ) -> Result<Vec<MarginInterestRecord>, MarginSourceError> {
            self.interest_calls
                .lock()
                .expect("lock interest calls")
                .push(date);
            if self.interest_failure == Some(date) {
                return Err(MarginSourceError::Failed("sample failure".into()));
            }
            Ok(self
                .interest_record
                .iter()
                .filter(|record| record.date == date)
                .cloned()
                .collect())
        }

        async fn fetch_margin_alert(
            &self,
            date: NaiveDate,
        ) -> Result<Vec<MarginAlertRecord>, MarginSourceError> {
            self.alert_calls
                .lock()
                .expect("lock alert calls")
                .push(date);
            if self.alert_failure == Some(date) {
                return Err(MarginSourceError::Failed("sample failure".into()));
            }
            Ok(self
                .alert_record
                .iter()
                .filter(|record| record.pub_date == date)
                .cloned()
                .collect())
        }

        fn fetchable_range(&self, _today: NaiveDate) -> Option<DateRange> {
            self.range.clone()
        }
    }

    fn ymd(year: i32, month: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(year, month, day).expect("valid date")
    }

    fn make_interest(date: NaiveDate) -> MarginInterestRecord {
        MarginInterestRecord {
            date,
            code: "12340".into(),
            iss_type: 1,
            shrt_vol: 1,
            long_vol: 2,
            shrt_neg_vol: 0,
            long_neg_vol: 0,
            shrt_std_vol: 1,
            long_std_vol: 2,
            shrt_val: None,
            long_val: None,
            shrt_neg_val: None,
            long_neg_val: None,
            shrt_std_val: None,
            long_std_val: None,
        }
    }

    fn make_alert(pub_date: NaiveDate, app_date: NaiveDate) -> MarginAlertRecord {
        MarginAlertRecord {
            pub_date,
            code: "12340".into(),
            app_date,
            pub_reason: PubReason {
                restricted: false,
                daily_publication: false,
                monitoring: false,
                restricted_by_jsf: false,
                precaution_by_jsf: false,
                unclear_or_sec_on_alert: false,
            },
            shrt_out: 1,
            long_out: 2,
            shrt_out_chg: None,
            long_out_chg: None,
            shrt_out_ratio: None,
            long_out_ratio: None,
            sl_ratio: None,
            shrt_neg_out: 0,
            shrt_std_out: 1,
            long_neg_out: 0,
            long_std_out: 2,
            tse_mrgn_reg_cls: "001".into(),
        }
    }

    fn use_cases(repository: Arc<FakeMarginRepository>) -> MarginUseCases {
        MarginUseCases::new(repository)
    }

    struct ExistingStrategyScopeSource;

    #[async_trait]
    impl StrategyScopeSource for ExistingStrategyScopeSource {
        async fn existing_ids(
            &self,
            ids: &[Uuid],
        ) -> Result<std::collections::HashSet<Uuid>, StrategyScopeSourceError> {
            Ok(ids.iter().copied().collect())
        }
    }

    async fn scope() -> StrategyScope {
        StrategyScope::verify(Uuid::new_v4(), &ExistingStrategyScopeSource)
            .await
            .expect("existing strategy")
    }

    #[rstest]
    #[case::refetches_30_days(ymd(2024, 2, 1), ymd(2024, 1, 1), ymd(2024, 2, 3), ymd(2024, 1, 2))]
    #[case::clamps_to_source_range(
        ymd(2024, 1, 2),
        ymd(2024, 1, 1),
        ymd(2024, 1, 3),
        ymd(2024, 1, 1)
    )]
    #[tokio::test]
    async fn ingest_starts_at_the_latest_date_lookback_clamped_to_source_range(
        #[case] latest: NaiveDate,
        #[case] range_from: NaiveDate,
        #[case] range_to: NaiveDate,
        #[case] expected_start: NaiveDate,
    ) {
        let repository = Arc::new(FakeMarginRepository::new());
        repository
            .upsert_margin_interest(vec![make_interest(latest)])
            .await
            .expect("seed interest");
        repository
            .upsert_margin_alert(vec![make_alert(latest, latest)])
            .await
            .expect("seed alert");
        let source = FakeMarginSource {
            range: Some(DateRange {
                from: range_from,
                to: range_to,
            }),
            ..FakeMarginSource::default()
        };

        let stats = use_cases(repository)
            .ingest(&source, range_to)
            .await
            .expect("ingest");
        let dates = (0..=(range_to - expected_start).num_days())
            .map(|offset| expected_start + chrono::Duration::days(offset))
            .collect::<Vec<_>>();
        let actual = (
            source
                .interest_calls
                .lock()
                .expect("interest calls")
                .clone(),
            source.alert_calls.lock().expect("alert calls").clone(),
            stats,
        );
        let expected_stats = IngestStats {
            days_fetched: dates.len(),
            rows_upserted: 0,
        };

        assert_eq!(
            actual,
            (dates.clone(), dates, (expected_stats, expected_stats))
        );
    }

    #[tokio::test]
    async fn ingest_backfills_both_record_types_and_counts_successful_rows() {
        let repository = Arc::new(FakeMarginRepository::new());
        let expected_interest = make_interest(ymd(2024, 1, 1));
        let expected_alert = make_alert(ymd(2024, 1, 1), ymd(2024, 1, 1));
        let source = FakeMarginSource {
            range: Some(DateRange {
                from: ymd(2024, 1, 1),
                to: ymd(2024, 1, 1),
            }),
            interest_record: Some(expected_interest.clone()),
            alert_record: Some(expected_alert.clone()),
            ..FakeMarginSource::default()
        };

        let stats = use_cases(repository.clone())
            .ingest(&source, ymd(2024, 1, 1))
            .await
            .expect("ingest");
        let state = (
            stats,
            repository
                .interest
                .lock()
                .expect("interest records")
                .clone(),
            repository.alerts.lock().expect("alert records").clone(),
        );

        assert_eq!(
            state,
            (
                (
                    IngestStats {
                        days_fetched: 1,
                        rows_upserted: 1,
                    },
                    IngestStats {
                        days_fetched: 1,
                        rows_upserted: 1,
                    },
                ),
                vec![expected_interest],
                vec![expected_alert],
            ),
        );
    }

    #[tokio::test]
    async fn ingest_fetches_more_than_30_days_on_initial_backfill() {
        let repository = Arc::new(FakeMarginRepository::new());
        let target = ymd(2024, 2, 5);
        let source = FakeMarginSource {
            range: Some(DateRange {
                from: ymd(2024, 1, 1),
                to: ymd(2024, 2, 10),
            }),
            interest_record: Some(make_interest(target)),
            ..FakeMarginSource::default()
        };

        let stats = use_cases(repository.clone())
            .ingest(&source, ymd(2024, 2, 10))
            .await
            .expect("ingest");
        let actual = (
            source
                .interest_calls
                .lock()
                .expect("interest calls")
                .clone(),
            stats,
            repository
                .interest
                .lock()
                .expect("interest records")
                .clone(),
        );
        let expected_dates = (0..41)
            .map(|offset| ymd(2024, 1, 1) + chrono::Duration::days(offset))
            .collect::<Vec<_>>();

        assert_eq!(
            actual,
            (
                expected_dates,
                (
                    IngestStats {
                        days_fetched: 41,
                        rows_upserted: 1,
                    },
                    IngestStats {
                        days_fetched: 41,
                        rows_upserted: 0,
                    },
                ),
                vec![make_interest(target)],
            ),
        );
    }

    #[tokio::test]
    async fn ingest_continues_after_a_failed_day() {
        let repository = Arc::new(FakeMarginRepository::new());
        let source = FakeMarginSource {
            range: Some(DateRange {
                from: ymd(2024, 1, 1),
                to: ymd(2024, 1, 3),
            }),
            interest_failure: Some(ymd(2024, 1, 2)),
            alert_failure: Some(ymd(2024, 1, 2)),
            ..FakeMarginSource::default()
        };

        let stats = use_cases(repository.clone())
            .ingest(&source, ymd(2024, 1, 3))
            .await
            .expect("ingest");
        let calls = (
            source
                .interest_calls
                .lock()
                .expect("interest calls")
                .clone(),
            source.alert_calls.lock().expect("alert calls").clone(),
            stats,
        );
        assert_eq!(
            calls,
            (
                vec![ymd(2024, 1, 1), ymd(2024, 1, 2), ymd(2024, 1, 3)],
                vec![ymd(2024, 1, 1), ymd(2024, 1, 2), ymd(2024, 1, 3)],
                (
                    IngestStats {
                        days_fetched: 2,
                        rows_upserted: 0,
                    },
                    IngestStats {
                        days_fetched: 2,
                        rows_upserted: 0,
                    },
                ),
            ),
        );
    }

    #[tokio::test]
    async fn ingest_skips_when_range_is_unavailable() {
        let source = FakeMarginSource::default();
        let stats = use_cases(Arc::new(FakeMarginRepository::new()))
            .ingest(&source, ymd(2024, 1, 3))
            .await
            .expect("empty range");

        assert_eq!(
            (
                source
                    .interest_calls
                    .lock()
                    .expect("interest calls")
                    .clone(),
                source.alert_calls.lock().expect("alert calls").clone(),
                stats,
            ),
            (
                Vec::new(),
                Vec::new(),
                (IngestStats::default(), IngestStats::default()),
            ),
        );
    }

    #[tokio::test]
    async fn ingest_continues_after_a_day_fails_to_save() {
        let repository = Arc::new(FakeMarginRepository::new());
        repository
            .fail_interest_upserts_on
            .lock()
            .expect("interest failure dates")
            .insert(ymd(2024, 1, 2));
        repository
            .fail_alert_upserts_on
            .lock()
            .expect("alert failure dates")
            .insert(ymd(2024, 1, 2));
        let source = FakeMarginSource {
            range: Some(DateRange {
                from: ymd(2024, 1, 1),
                to: ymd(2024, 1, 3),
            }),
            interest_record: Some(make_interest(ymd(2024, 1, 2))),
            alert_record: Some(make_alert(ymd(2024, 1, 2), ymd(2024, 1, 2))),
            ..FakeMarginSource::default()
        };

        let stats = use_cases(repository.clone())
            .ingest(&source, ymd(2024, 1, 3))
            .await
            .expect("ingest");
        let actual = (
            source
                .interest_calls
                .lock()
                .expect("interest calls")
                .clone(),
            source.alert_calls.lock().expect("alert calls").clone(),
            stats,
            repository
                .interest
                .lock()
                .expect("interest records")
                .clone(),
            repository.alerts.lock().expect("alert records").clone(),
        );

        assert_eq!(
            actual,
            (
                vec![ymd(2024, 1, 1), ymd(2024, 1, 2), ymd(2024, 1, 3)],
                vec![ymd(2024, 1, 1), ymd(2024, 1, 2), ymd(2024, 1, 3)],
                (
                    IngestStats {
                        days_fetched: 2,
                        rows_upserted: 0,
                    },
                    IngestStats {
                        days_fetched: 2,
                        rows_upserted: 0,
                    },
                ),
                vec![],
                vec![],
            ),
        );
    }

    #[tokio::test]
    async fn read_rejects_a_reversed_date_range() {
        let error = use_cases(Arc::new(FakeMarginRepository::new()))
            .read(
                scope().await,
                MarginQuery {
                    symbol: "1234".into(),
                    from: Some(ymd(2024, 1, 2)),
                    to: Some(ymd(2024, 1, 1)),
                    limit: 1,
                },
            )
            .await
            .expect_err("reversed range");

        assert_eq!(error.to_string(), "from must be on or before to");
    }

    #[tokio::test]
    async fn read_returns_records_for_the_query_without_strategy_filtering() {
        let repository = Arc::new(FakeMarginRepository::new());
        let interest = make_interest(ymd(2024, 1, 1));
        repository
            .upsert_margin_interest(vec![interest.clone()])
            .await
            .expect("seed interest");
        let expected = MarginReadResult {
            interest: vec![interest],
            alerts: vec![],
        };

        let result = use_cases(repository.clone())
            .read(
                scope().await,
                MarginQuery {
                    symbol: "1234".into(),
                    from: None,
                    to: None,
                    limit: 50,
                },
            )
            .await
            .expect("read");

        assert_eq!(result, expected);
    }
}
