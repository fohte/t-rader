use chrono::NaiveDate;
use core_domain::business_day::latest_business_day;

use crate::daily_bar_source::DateRange;
use crate::earnings_schedule_source::EarningsScheduleSource;

use super::error::EarningsScheduleUseCaseError;
use super::repository::SharedEarningsScheduleRepository;

const LOOKBACK_DAYS: i64 = 30;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct EarningsScheduleIngestStats {
    pub days_attempted: usize,
    pub upserted: usize,
}

#[derive(Clone)]
pub struct EarningsScheduleUseCases {
    repository: SharedEarningsScheduleRepository,
}

impl EarningsScheduleUseCases {
    pub fn new(repository: SharedEarningsScheduleRepository) -> Self {
        Self { repository }
    }

    pub async fn run_ingest_cycle(
        &self,
        source: &dyn EarningsScheduleSource,
        today: NaiveDate,
    ) -> Result<EarningsScheduleIngestStats, EarningsScheduleUseCaseError> {
        let Some(source_range) = source.fetchable_range(today) else {
            tracing::debug!("決算発表予定日の取得可能範囲がないため取り込みをスキップします");
            return Ok(EarningsScheduleIngestStats::default());
        };

        let latest_stored = self.repository.latest_published_date().await?;
        let (from, to) = fetch_range(&source_range, latest_stored);

        let mut stats = EarningsScheduleIngestStats::default();
        let mut date = from;
        while date <= to {
            if latest_business_day(date) == date {
                stats.days_attempted += 1;
                match source.fetch_earnings_schedules_by_date(date).await {
                    Ok(schedules) => match self.repository.upsert(schedules).await {
                        Ok(count) => stats.upserted += count,
                        Err(error) => {
                            tracing::warn!(%date, %error, "決算発表予定日の格納に失敗、この日をスキップします");
                        }
                    },
                    Err(error) => {
                        tracing::warn!(%date, %error, "決算発表予定日の取得に失敗、この日をスキップします");
                    }
                }
            }
            date += chrono::Duration::days(1);
        }

        Ok(stats)
    }
}

fn fetch_range(
    source_range: &DateRange,
    latest_stored: Option<NaiveDate>,
) -> (NaiveDate, NaiveDate) {
    let from = match latest_stored {
        Some(latest) => (latest - chrono::Duration::days(LOOKBACK_DAYS)).max(source_range.from),
        None => source_range.from,
    };
    (from, source_range.to)
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Arc;
    use std::sync::Mutex;

    use async_trait::async_trait;
    use chrono::NaiveDate;
    use core_domain::earnings_schedule::EarningsSchedule;
    use rstest::rstest;

    use crate::daily_bar_source::DateRange;
    use crate::earnings_schedule_source::{EarningsScheduleSource, EarningsScheduleSourceError};

    use super::EarningsScheduleUseCases;
    use crate::earnings_schedule::repository::{
        EarningsScheduleRepository, EarningsScheduleRepositoryError,
        SharedEarningsScheduleRepository,
    };
    use crate::persistence::PersistenceError;

    fn date(year: i32, month: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(year, month, day).expect("valid date")
    }

    #[derive(Default)]
    struct FakeEarningsScheduleSource {
        range: Option<DateRange>,
        schedules: HashMap<NaiveDate, Vec<EarningsSchedule>>,
        failed_date: Option<NaiveDate>,
        calls: Mutex<Vec<NaiveDate>>,
    }

    #[async_trait]
    impl EarningsScheduleSource for FakeEarningsScheduleSource {
        async fn fetch_earnings_schedules_by_date(
            &self,
            published_date: NaiveDate,
        ) -> Result<Vec<EarningsSchedule>, EarningsScheduleSourceError> {
            self.calls.lock().expect("calls lock").push(published_date);
            if self.failed_date == Some(published_date) {
                return Err(EarningsScheduleSourceError::Failed(
                    "synthetic failure".into(),
                ));
            }
            Ok(self
                .schedules
                .get(&published_date)
                .cloned()
                .unwrap_or_default())
        }

        fn fetchable_range(&self, _today: NaiveDate) -> Option<DateRange> {
            self.range.clone()
        }
    }

    #[derive(Default)]
    struct FakeEarningsScheduleRepository {
        latest_published_date: Option<NaiveDate>,
        schedules: Mutex<Vec<EarningsSchedule>>,
        fail_upsert_date: Option<NaiveDate>,
    }

    #[async_trait]
    impl EarningsScheduleRepository for FakeEarningsScheduleRepository {
        async fn latest_published_date(
            &self,
        ) -> Result<Option<NaiveDate>, EarningsScheduleRepositoryError> {
            Ok(self.latest_published_date)
        }

        async fn upsert(
            &self,
            schedules: Vec<EarningsSchedule>,
        ) -> Result<usize, EarningsScheduleRepositoryError> {
            if schedules
                .iter()
                .any(|schedule| Some(schedule.published_date) == self.fail_upsert_date)
            {
                return Err(EarningsScheduleRepositoryError::Database(
                    PersistenceError::Database("synthetic failure".into()),
                ));
            }
            let count = schedules.len();
            let mut stored = self.schedules.lock().expect("schedules lock");
            for schedule in schedules {
                stored.retain(|saved| {
                    saved.code != schedule.code
                        || saved.fiscal_quarter_name != schedule.fiscal_quarter_name
                        || saved.published_date != schedule.published_date
                });
                stored.push(schedule);
            }
            Ok(count)
        }
    }

    fn make_schedule(published_date: NaiveDate) -> EarningsSchedule {
        EarningsSchedule {
            code: "ZZ999".into(),
            fiscal_quarter_name: "FY-QX".into(),
            published_date,
            scheduled_date: None,
            fiscal_year_end: "1231".into(),
            company_name: "架空社".into(),
            company_name_en: "Synthetic Company".into(),
        }
    }

    fn make_use_cases(
        source: FakeEarningsScheduleSource,
        repository: FakeEarningsScheduleRepository,
    ) -> (
        EarningsScheduleUseCases,
        Arc<FakeEarningsScheduleSource>,
        Arc<FakeEarningsScheduleRepository>,
    ) {
        let source = Arc::new(source);
        let repository = Arc::new(repository);
        let use_cases =
            EarningsScheduleUseCases::new(repository.clone() as SharedEarningsScheduleRepository);
        (use_cases, source, repository)
    }

    #[rstest]
    #[case::starts_at_source_range(None, (date(2042, 7, 6), date(2042, 8, 20)))]
    #[case::uses_lookback(
        Some(date(2042, 8, 15)),
        (date(2042, 7, 16), date(2042, 8, 20))
    )]
    #[case::clamps_lookback(
        Some(date(2042, 7, 20)),
        (date(2042, 7, 6), date(2042, 8, 20))
    )]
    fn fetch_range_uses_source_window_and_lookback(
        #[case] latest_stored: Option<NaiveDate>,
        #[case] expected: (NaiveDate, NaiveDate),
    ) {
        let source_range = DateRange {
            from: date(2042, 7, 6),
            to: date(2042, 8, 20),
        };

        assert_eq!(super::fetch_range(&source_range, latest_stored), expected);
    }

    #[tokio::test]
    async fn run_ingest_cycle_skips_weekends_and_upserts_fetched_schedules() {
        let saturday = date(2042, 7, 5);
        let monday = date(2042, 7, 7);
        let source = FakeEarningsScheduleSource {
            range: Some(DateRange {
                from: saturday,
                to: monday,
            }),
            schedules: HashMap::from([(monday, vec![make_schedule(monday)])]),
            ..Default::default()
        };
        let repository = FakeEarningsScheduleRepository::default();
        let (use_cases, source, repository) = make_use_cases(source, repository);

        let stats = use_cases
            .run_ingest_cycle(source.as_ref(), date(2042, 7, 8))
            .await
            .expect("cycle succeeds");

        assert_eq!(
            (
                stats,
                source.calls.lock().expect("calls lock").clone(),
                repository.schedules.lock().expect("schedules lock").clone(),
            ),
            (
                crate::earnings_schedule::EarningsScheduleIngestStats {
                    days_attempted: 1,
                    upserted: 1,
                },
                vec![monday],
                vec![make_schedule(monday)],
            ),
        );
    }

    #[tokio::test]
    async fn run_ingest_cycle_continues_after_a_date_fails_to_fetch() {
        let failed_date = date(2042, 7, 7);
        let successful_date = date(2042, 7, 8);
        let source = FakeEarningsScheduleSource {
            range: Some(DateRange {
                from: failed_date,
                to: successful_date,
            }),
            schedules: HashMap::from([(successful_date, vec![make_schedule(successful_date)])]),
            failed_date: Some(failed_date),
            ..Default::default()
        };
        let repository = FakeEarningsScheduleRepository::default();
        let (use_cases, source, repository) = make_use_cases(source, repository);

        let stats = use_cases
            .run_ingest_cycle(source.as_ref(), successful_date)
            .await
            .expect("cycle succeeds");

        assert_eq!(
            (
                stats,
                source.calls.lock().expect("calls lock").clone(),
                repository.schedules.lock().expect("schedules lock").clone(),
            ),
            (
                crate::earnings_schedule::EarningsScheduleIngestStats {
                    days_attempted: 2,
                    upserted: 1,
                },
                vec![failed_date, successful_date],
                vec![make_schedule(successful_date)],
            ),
        );
    }

    #[tokio::test]
    async fn run_ingest_cycle_continues_after_a_date_fails_to_upsert() {
        let failed_date = date(2042, 7, 7);
        let successful_date = date(2042, 7, 8);
        let source = FakeEarningsScheduleSource {
            range: Some(DateRange {
                from: failed_date,
                to: successful_date,
            }),
            schedules: HashMap::from([
                (failed_date, vec![make_schedule(failed_date)]),
                (successful_date, vec![make_schedule(successful_date)]),
            ]),
            ..Default::default()
        };
        let repository = FakeEarningsScheduleRepository {
            fail_upsert_date: Some(failed_date),
            ..Default::default()
        };
        let (use_cases, source, repository) = make_use_cases(source, repository);

        let stats = use_cases
            .run_ingest_cycle(source.as_ref(), successful_date)
            .await
            .expect("cycle succeeds");

        assert_eq!(
            (
                stats,
                source.calls.lock().expect("calls lock").clone(),
                repository.schedules.lock().expect("schedules lock").clone(),
            ),
            (
                crate::earnings_schedule::EarningsScheduleIngestStats {
                    days_attempted: 2,
                    upserted: 1,
                },
                vec![failed_date, successful_date],
                vec![make_schedule(successful_date)],
            ),
        );
    }
}
