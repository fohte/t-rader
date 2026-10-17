use chrono::{DateTime, Duration, Utc};

use super::error::BarsUseCaseError;
use super::hourly_bar_bucket_start;
use super::types::UsStockMinuteBarsDownsampleStats;
use super::us_stock_hourly_history_cutoff;
use super::use_cases::BarsUseCases;

const DOWNSAMPLE_BATCH_DAYS: i64 = 7;

impl BarsUseCases {
    pub async fn downsample_old_us_stock_minute_bars(
        &self,
        now: DateTime<Utc>,
    ) -> Result<UsStockMinuteBarsDownsampleStats, BarsUseCaseError> {
        let cutoff = us_stock_hourly_history_cutoff(now);
        let Some(oldest_bar) = self
            .repository
            .find_earliest_us_stock_minute_bar_before(cutoff)
            .await?
        else {
            return Ok(UsStockMinuteBarsDownsampleStats::default());
        };

        let mut batch_from = hourly_bar_bucket_start(oldest_bar);
        let mut stats = UsStockMinuteBarsDownsampleStats::default();
        let instrument_ids = self.repository.find_us_stock_instrument_ids().await?;
        while batch_from < cutoff {
            let batch_until = batch_from
                .checked_add_signed(Duration::days(DOWNSAMPLE_BATCH_DAYS))
                .unwrap_or(cutoff);
            for instrument_id in &instrument_ids {
                let batch_result = async {
                    let transaction = self.unit_of_work.begin().await?;
                    let batch_stats = self
                        .repository
                        .downsample_us_stock_minute_bar_range(
                            &transaction,
                            instrument_id,
                            batch_from,
                            batch_until,
                            cutoff,
                        )
                        .await?;
                    self.unit_of_work.commit(transaction).await?;
                    Ok::<_, BarsUseCaseError>(batch_stats)
                }
                .await;
                let batch_stats = match batch_result {
                    Ok(batch_stats) => batch_stats,
                    Err(error) => {
                        tracing::error!(
                            instrument_id = %instrument_id,
                            error = %error,
                            "米国株の分足を集計できませんでした"
                        );
                        return Err(error);
                    }
                };

                stats.hourly_bars_inserted += batch_stats.hourly_bars_inserted;
                stats.minute_bars_deleted += batch_stats.minute_bars_deleted;
            }
            batch_from = batch_until;
        }

        Ok(stats)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use chrono::{DateTime, Utc};
    use core_domain::bar::{Bar, Timeframe};
    use rstest::{fixture, rstest};
    use rust_decimal::Decimal;

    use crate::unit_of_work::{FakeUnitOfWork, SharedUnitOfWork};

    use super::super::{BarsUseCases, FakeBarsRepository};

    fn date(value: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(value)
            .expect("valid timestamp")
            .to_utc()
    }

    fn minute_bar(instrument_id: &str, timestamp: DateTime<Utc>) -> Bar {
        Bar {
            instrument_id: instrument_id.to_owned(),
            timeframe: Timeframe::Minute,
            timestamp,
            open: Decimal::ONE,
            high: Decimal::ONE,
            low: Decimal::ONE,
            close: Decimal::ONE,
            volume: 1,
            adjustment_factor: Decimal::ONE,
        }
    }

    struct DownsampleFixture {
        use_cases: BarsUseCases,
        unit_of_work: Arc<FakeUnitOfWork>,
        repository: Arc<FakeBarsRepository>,
    }

    #[fixture]
    async fn downsample_fixture() -> DownsampleFixture {
        let unit_of_work = Arc::new(FakeUnitOfWork::new());
        let repository = Arc::new(FakeBarsRepository::new());
        repository
            .seed_us_stock_instrument_ids(vec![
                "US:DEMO-ALPHA".to_owned(),
                "US:DEMO-BETA".to_owned(),
            ])
            .await;
        let shared_unit_of_work: SharedUnitOfWork = unit_of_work.clone();
        let use_cases = BarsUseCases::new(shared_unit_of_work, repository.clone());
        DownsampleFixture {
            use_cases,
            unit_of_work,
            repository,
        }
    }

    #[rstest]
    #[tokio::test]
    #[awt]
    async fn downsample_processes_aged_minutes_in_hour_aligned_weekly_transactions(
        #[future] downsample_fixture: DownsampleFixture,
    ) {
        let repository = &downsample_fixture.repository;
        repository
            .seed_bars(vec![
                minute_bar("US:DEMO-ALPHA", date("2038-12-20T03:05:00Z")),
                minute_bar("US:DEMO-BETA", date("2038-12-21T03:05:00Z")),
                minute_bar("US:DEMO-ALPHA", date("2038-12-29T10:05:00Z")),
                minute_bar("US:DEMO-ALPHA", date("2039-01-01T12:05:00Z")),
                minute_bar("JP-DEMO", date("2038-12-20T03:05:00Z")),
            ])
            .await;

        let result = downsample_fixture
            .use_cases
            .downsample_old_us_stock_minute_bars(date("2040-01-01T12:00:00Z"))
            .await;
        let downsample_instruments = repository
            .minute_bar_downsample_instruments
            .lock()
            .await
            .clone();
        let downsample_ranges = repository.minute_bar_downsample_ranges.lock().await.clone();
        let cutoffs = repository
            .minute_bar_downsample_cutoffs
            .lock()
            .await
            .clone();
        let write_transaction_ids = repository.write_transaction_ids.lock().await.clone();
        let begun = downsample_fixture.unit_of_work.begun.lock().await.clone();
        let committed = downsample_fixture
            .unit_of_work
            .committed
            .lock()
            .await
            .clone();

        assert_eq!(
            (
                result.map_err(|_| ()),
                downsample_instruments,
                downsample_ranges,
                cutoffs,
                begun.len(),
                write_transaction_ids == begun,
                committed == begun,
            ),
            (
                Ok(Default::default()),
                vec![
                    "US:DEMO-ALPHA".to_owned(),
                    "US:DEMO-BETA".to_owned(),
                    "US:DEMO-ALPHA".to_owned(),
                    "US:DEMO-BETA".to_owned(),
                ],
                vec![
                    (date("2038-12-20T03:00:00Z"), date("2038-12-27T03:00:00Z"),),
                    (date("2038-12-20T03:00:00Z"), date("2038-12-27T03:00:00Z"),),
                    (date("2038-12-27T03:00:00Z"), date("2039-01-03T03:00:00Z"),),
                    (date("2038-12-27T03:00:00Z"), date("2039-01-03T03:00:00Z"),),
                ],
                vec![
                    date("2039-01-01T12:00:00Z"),
                    date("2039-01-01T12:00:00Z"),
                    date("2039-01-01T12:00:00Z"),
                    date("2039-01-01T12:00:00Z"),
                ],
                4,
                true,
                true,
            ),
        );
    }

    #[rstest]
    #[tokio::test]
    #[awt]
    async fn downsample_failure_keeps_prior_instrument_commits(
        #[future] downsample_fixture: DownsampleFixture,
    ) {
        let repository = &downsample_fixture.repository;
        repository
            .seed_bars(vec![
                minute_bar("US:DEMO-ALPHA", date("2038-12-20T03:05:00Z")),
                minute_bar("US:DEMO-BETA", date("2038-12-20T03:05:00Z")),
            ])
            .await;
        repository
            .fail_minute_bar_downsampling_for("US:DEMO-BETA", "injected database failure")
            .await;

        let result = downsample_fixture
            .use_cases
            .downsample_old_us_stock_minute_bars(date("2040-01-01T12:00:00Z"))
            .await
            .map_err(|_| ());
        let downsample_instruments = repository
            .minute_bar_downsample_instruments
            .lock()
            .await
            .clone();
        let begun = downsample_fixture.unit_of_work.begun.lock().await.clone();
        let committed = downsample_fixture
            .unit_of_work
            .committed
            .lock()
            .await
            .clone();
        let write_transaction_ids = repository.write_transaction_ids.lock().await.clone();
        let transaction_results = begun
            .iter()
            .map(|transaction_id| {
                (
                    write_transaction_ids.contains(transaction_id),
                    committed.contains(transaction_id),
                )
            })
            .collect::<Vec<_>>();

        assert_eq!(
            (result, downsample_instruments, transaction_results),
            (
                Err(()),
                vec!["US:DEMO-ALPHA".to_owned(), "US:DEMO-BETA".to_owned()],
                vec![(true, true), (true, false)],
            ),
        );
    }
}
