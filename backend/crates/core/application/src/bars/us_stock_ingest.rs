use chrono::{DateTime, Months, TimeZone, Utc};
use core_domain::bar::Timeframe;

use super::error::BarsUseCaseError;
use super::types::{UsStockBarTarget, UsStockBarsIngestStats};
use super::us_stock_source::{UsStockBarQuery, UsStockBarSource};
use super::use_cases::BarsUseCases;

const SYMBOLS_PER_REQUEST: usize = 100;
const MINUTE_HISTORY_MONTHS: u32 = 24;

impl BarsUseCases {
    pub async fn ingest_us_stock_bars(
        &self,
        source: &dyn UsStockBarSource,
    ) -> Result<UsStockBarsIngestStats, BarsUseCaseError> {
        self.ingest_us_stock_bars_at(source, Utc::now()).await
    }

    async fn ingest_us_stock_bars_at(
        &self,
        source: &dyn UsStockBarSource,
        now: DateTime<Utc>,
    ) -> Result<UsStockBarsIngestStats, BarsUseCaseError> {
        let targets = self.repository.find_us_stock_bar_targets().await?;
        let mut stats = UsStockBarsIngestStats {
            symbols_attempted: targets.len(),
            ..UsStockBarsIngestStats::default()
        };
        let daily_history_start = Utc.timestamp_opt(0, 0).single().unwrap_or(now);
        let minute_history_start = now
            .checked_sub_months(Months::new(MINUTE_HISTORY_MONTHS))
            .unwrap_or(now);

        self.ingest_timeframe(
            source,
            &targets,
            Timeframe::Daily,
            daily_history_start,
            now,
            &mut stats,
        )
        .await?;
        self.ingest_timeframe(
            source,
            &targets,
            Timeframe::Minute,
            minute_history_start,
            now,
            &mut stats,
        )
        .await?;

        Ok(stats)
    }

    async fn ingest_timeframe(
        &self,
        source: &dyn UsStockBarSource,
        targets: &[UsStockBarTarget],
        timeframe: Timeframe,
        history_start: DateTime<Utc>,
        to: DateTime<Utc>,
        stats: &mut UsStockBarsIngestStats,
    ) -> Result<(), BarsUseCaseError> {
        let mut missing_history = Vec::new();
        let mut existing_history = Vec::new();
        for target in targets {
            match target.latest_bar(timeframe) {
                Some(timestamp) => existing_history.push((target.instrument_id.clone(), timestamp)),
                None => missing_history.push(target.instrument_id.clone()),
            }
        }

        for instrument_ids in missing_history.chunks(SYMBOLS_PER_REQUEST) {
            self.ingest_batch(source, instrument_ids, timeframe, history_start, to, stats)
                .await?;
        }

        for target_batch in existing_history.chunks(SYMBOLS_PER_REQUEST) {
            let from = target_batch
                .iter()
                .map(|(_, timestamp)| *timestamp)
                .min()
                .unwrap_or(history_start);
            let instrument_ids = target_batch
                .iter()
                .map(|(instrument_id, _)| instrument_id.clone())
                .collect::<Vec<_>>();
            self.ingest_batch(source, &instrument_ids, timeframe, from, to, stats)
                .await?;
        }

        Ok(())
    }

    async fn ingest_batch(
        &self,
        source: &dyn UsStockBarSource,
        instrument_ids: &[String],
        timeframe: Timeframe,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
        stats: &mut UsStockBarsIngestStats,
    ) -> Result<(), BarsUseCaseError> {
        let mut page_token = None;
        loop {
            stats.requests_attempted += 1;
            let page = source
                .fetch_page(&UsStockBarQuery {
                    instrument_ids: instrument_ids.to_vec(),
                    timeframe,
                    from,
                    to,
                    page_token: page_token.clone(),
                })
                .await?;

            if !page.bars.is_empty() {
                let count = page.bars.len();
                let transaction = self.unit_of_work.begin().await?;
                match timeframe {
                    Timeframe::Daily => {
                        self.repository.upsert_bars(&transaction, page.bars).await?;
                        stats.daily_bars_upserted += count;
                    }
                    Timeframe::Minute => {
                        self.repository
                            .upsert_minute_bars(&transaction, page.bars)
                            .await?;
                        stats.minute_bars_upserted += count;
                    }
                    _ => {
                        return Err(crate::bars::error::BarsUseCaseError::UsStockBarSource(
                            crate::bars::us_stock_source::UsStockBarSourceError::Failed(format!(
                                "unsupported US stock bar timeframe: {timeframe}"
                            )),
                        ));
                    }
                }
                self.unit_of_work.commit(transaction).await?;
            }

            let Some(next_page_token) = page.next_page_token else {
                return Ok(());
            };
            if page_token.as_ref() == Some(&next_page_token) {
                return Err(crate::bars::error::BarsUseCaseError::UsStockBarSource(
                    crate::bars::us_stock_source::UsStockBarSourceError::Failed(
                        "pagination token did not advance".to_owned(),
                    ),
                ));
            }
            page_token = Some(next_page_token);
        }
    }
}

#[cfg(all(test, feature = "test-support"))]
mod tests {
    use std::sync::Arc;

    use async_trait::async_trait;
    use chrono::{DateTime, TimeZone, Utc};
    use core_domain::bar::{Bar, Timeframe};
    use rstest::{fixture, rstest};
    use rust_decimal::Decimal;
    use tokio::sync::Mutex;

    use crate::bars::{
        BarsUseCases, FakeBarsRepository, UsStockBarPage, UsStockBarQuery, UsStockBarSource,
        UsStockBarSourceError, UsStockBarsIngestStats,
    };
    use crate::unit_of_work::{FakeUnitOfWork, SharedUnitOfWork};

    use super::MINUTE_HISTORY_MONTHS;

    struct Harness {
        use_cases: BarsUseCases,
        repository: Arc<FakeBarsRepository>,
        source: FakeSource,
    }

    #[fixture]
    fn harness() -> Harness {
        let repository = Arc::new(FakeBarsRepository::new());
        let repository_shared = repository.clone();
        let repository_shared_trait: crate::bars::SharedBarsRepository = repository_shared;
        let unit_of_work = Arc::new(FakeUnitOfWork::new());
        let unit_of_work_shared: SharedUnitOfWork = unit_of_work;
        Harness {
            use_cases: BarsUseCases::new(unit_of_work_shared, repository_shared_trait),
            repository,
            source: FakeSource::default(),
        }
    }

    #[derive(Default)]
    struct FakeSource {
        calls: Mutex<Vec<UsStockBarQuery>>,
    }

    #[async_trait]
    impl UsStockBarSource for FakeSource {
        async fn fetch_page(
            &self,
            query: &UsStockBarQuery,
        ) -> Result<UsStockBarPage, UsStockBarSourceError> {
            self.calls.lock().await.push(query.clone());
            let bars = if query.page_token.is_some() {
                Vec::new()
            } else {
                query
                    .instrument_ids
                    .iter()
                    .map(|instrument_id| bar(instrument_id, query.timeframe, query.to))
                    .collect()
            };
            let next_page_token = (query.timeframe == Timeframe::Daily
                && query.instrument_ids == vec!["US:QZ7"]
                && query.page_token.is_none())
            .then(|| "synthetic-page".to_owned());
            Ok(UsStockBarPage {
                bars,
                next_page_token,
            })
        }
    }

    fn date(value: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(value)
            .map(|timestamp| timestamp.to_utc())
            .unwrap_or_default()
    }

    fn bar(instrument_id: &str, timeframe: Timeframe, timestamp: DateTime<Utc>) -> Bar {
        Bar {
            instrument_id: instrument_id.to_owned(),
            timeframe,
            timestamp,
            open: Decimal::new(101, 1),
            high: Decimal::new(111, 1),
            low: Decimal::new(91, 1),
            close: Decimal::new(105, 1),
            volume: 25,
        }
    }

    #[rstest]
    #[tokio::test]
    async fn ingest_backfills_missing_symbols_and_continues_from_saved_bars(harness: Harness) {
        harness
            .repository
            .seed_us_stock_targets(vec!["US:QZ7".to_owned(), "US:QZ-7".to_owned()])
            .await;
        harness
            .repository
            .seed_bars(vec![
                bar("US:QZ-7", Timeframe::Daily, date("2040-01-01T00:00:00Z")),
                bar("US:QZ-7", Timeframe::Minute, date("2040-01-02T11:59:00Z")),
            ])
            .await;
        let now = Utc
            .with_ymd_and_hms(2040, 1, 2, 12, 0, 0)
            .single()
            .unwrap_or_default();
        let stats = harness
            .use_cases
            .ingest_us_stock_bars_at(&harness.source, now)
            .await
            .expect("US stock bar ingestion succeeds");
        let calls = harness.source.calls.lock().await.clone();
        let bars = harness.repository.bars.lock().await.clone();
        let expected_minute_start = now
            .checked_sub_months(chrono::Months::new(MINUTE_HISTORY_MONTHS))
            .unwrap_or(now);
        let expected = (
            UsStockBarsIngestStats {
                symbols_attempted: 2,
                requests_attempted: 5,
                daily_bars_upserted: 2,
                minute_bars_upserted: 2,
            },
            vec![
                UsStockBarQuery {
                    instrument_ids: vec!["US:QZ7".to_owned()],
                    timeframe: Timeframe::Daily,
                    from: date("1970-01-01T00:00:00Z"),
                    to: now,
                    page_token: None,
                },
                UsStockBarQuery {
                    instrument_ids: vec!["US:QZ7".to_owned()],
                    timeframe: Timeframe::Daily,
                    from: date("1970-01-01T00:00:00Z"),
                    to: now,
                    page_token: Some("synthetic-page".to_owned()),
                },
                UsStockBarQuery {
                    instrument_ids: vec!["US:QZ-7".to_owned()],
                    timeframe: Timeframe::Daily,
                    from: date("2040-01-01T00:00:00Z"),
                    to: now,
                    page_token: None,
                },
                UsStockBarQuery {
                    instrument_ids: vec!["US:QZ7".to_owned()],
                    timeframe: Timeframe::Minute,
                    from: expected_minute_start,
                    to: now,
                    page_token: None,
                },
                UsStockBarQuery {
                    instrument_ids: vec!["US:QZ-7".to_owned()],
                    timeframe: Timeframe::Minute,
                    from: date("2040-01-02T11:59:00Z"),
                    to: now,
                    page_token: None,
                },
            ],
            6,
        );

        assert_eq!((stats, calls, bars.len()), expected);
    }

    #[rstest]
    #[tokio::test]
    async fn groups_existing_symbols_by_their_own_latest_bar(harness: Harness) {
        let instrument_ids = (0..101)
            .map(|index| format!("US:QZ{index:03}"))
            .collect::<Vec<_>>();
        let recent_daily_bar = date("2040-01-01T00:00:00Z");
        let older_daily_bar = date("2039-01-01T00:00:00Z");
        let mut saved_bars = instrument_ids[..100]
            .iter()
            .map(|instrument_id| bar(instrument_id, Timeframe::Daily, recent_daily_bar))
            .collect::<Vec<_>>();
        saved_bars.push(bar(&instrument_ids[100], Timeframe::Daily, older_daily_bar));
        harness
            .repository
            .seed_us_stock_targets(instrument_ids.clone())
            .await;
        harness.repository.seed_bars(saved_bars).await;
        let now = Utc
            .with_ymd_and_hms(2040, 1, 2, 12, 0, 0)
            .single()
            .unwrap_or_default();
        let expected_minute_start = now
            .checked_sub_months(chrono::Months::new(MINUTE_HISTORY_MONTHS))
            .unwrap_or(now);

        harness
            .use_cases
            .ingest_us_stock_bars_at(&harness.source, now)
            .await
            .expect("US stock bar ingestion succeeds");

        let actual = harness.source.calls.lock().await.clone();
        let expected = vec![
            UsStockBarQuery {
                instrument_ids: instrument_ids[..100].to_vec(),
                timeframe: Timeframe::Daily,
                from: recent_daily_bar,
                to: now,
                page_token: None,
            },
            UsStockBarQuery {
                instrument_ids: vec![instrument_ids[100].clone()],
                timeframe: Timeframe::Daily,
                from: older_daily_bar,
                to: now,
                page_token: None,
            },
            UsStockBarQuery {
                instrument_ids: instrument_ids[..100].to_vec(),
                timeframe: Timeframe::Minute,
                from: expected_minute_start,
                to: now,
                page_token: None,
            },
            UsStockBarQuery {
                instrument_ids: vec![instrument_ids[100].clone()],
                timeframe: Timeframe::Minute,
                from: expected_minute_start,
                to: now,
                page_token: None,
            },
        ];

        assert_eq!(actual, expected);
    }
}
