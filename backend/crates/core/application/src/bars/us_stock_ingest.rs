use chrono::{DateTime, Months, TimeZone, Utc};
use core_domain::bar::Timeframe;

use super::error::BarsUseCaseError;
use super::types::{UsStockBarTarget, UsStockBarsIngestStats};
use super::us_stock_source::{UsStockBarQuery, UsStockBarSource, UsStockBarSourceError};
use super::use_cases::BarsUseCases;

const SYMBOLS_PER_REQUEST: usize = 100;
const MINUTE_HISTORY_MONTHS: u32 = 24;

#[derive(Clone, Copy)]
enum UsStockBarTimeframe {
    Daily,
    Minute,
}

impl UsStockBarTimeframe {
    fn timeframe(self) -> Timeframe {
        match self {
            Self::Daily => Timeframe::Daily,
            Self::Minute => Timeframe::Minute,
        }
    }

    fn latest_bar(self, target: &UsStockBarTarget) -> Option<DateTime<Utc>> {
        match self {
            Self::Daily => target.latest_daily_bar,
            Self::Minute => target.latest_minute_bar,
        }
    }
}

impl BarsUseCases {
    pub async fn ingest_us_stock_bars(
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
            UsStockBarTimeframe::Daily,
            daily_history_start,
            now,
            &mut stats,
        )
        .await?;
        self.ingest_timeframe(
            source,
            &targets,
            UsStockBarTimeframe::Minute,
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
        timeframe: UsStockBarTimeframe,
        history_start: DateTime<Utc>,
        to: DateTime<Utc>,
        stats: &mut UsStockBarsIngestStats,
    ) -> Result<(), BarsUseCaseError> {
        let mut missing_history = Vec::new();
        let mut existing_history = Vec::new();
        for target in targets {
            match timeframe.latest_bar(target) {
                Some(timestamp) => existing_history.push((target.instrument_id.clone(), timestamp)),
                None => missing_history.push(target.instrument_id.clone()),
            }
        }

        for instrument_ids in missing_history.chunks(SYMBOLS_PER_REQUEST) {
            self.ingest_batch(source, instrument_ids, timeframe, history_start, to, stats)
                .await?;
        }

        existing_history.sort_by_key(|(_, timestamp)| std::cmp::Reverse(*timestamp));
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
        timeframe: UsStockBarTimeframe,
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
                    timeframe: timeframe.timeframe(),
                    from,
                    to,
                    page_token: page_token.clone(),
                })
                .await?;

            if !page.bars.is_empty() {
                let count = page.bars.len();
                let transaction = self.unit_of_work.begin().await?;
                match timeframe {
                    UsStockBarTimeframe::Daily => {
                        self.repository.upsert_bars(&transaction, page.bars).await?;
                        stats.daily_bars_upserted += count;
                    }
                    UsStockBarTimeframe::Minute => {
                        self.repository
                            .upsert_minute_bars(&transaction, page.bars)
                            .await?;
                        stats.minute_bars_upserted += count;
                    }
                }
                self.unit_of_work.commit(transaction).await?;
            }

            let Some(next_page_token) = page.next_page_token else {
                return Ok(());
            };
            if page_token.as_ref() == Some(&next_page_token) {
                return Err(BarsUseCaseError::UsStockBarSource(
                    UsStockBarSourceError::Failed("pagination token did not advance".to_owned()),
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
        repeat_page_token: bool,
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
            let next_page_token = if self.repeat_page_token {
                Some(
                    query
                        .page_token
                        .clone()
                        .unwrap_or_else(|| "synthetic-page".to_owned()),
                )
            } else {
                (query.timeframe == Timeframe::Daily
                    && query.instrument_ids == vec!["US:QZ7"]
                    && query.page_token.is_none())
                .then(|| "synthetic-page".to_owned())
            };
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
            adjustment_factor: Decimal::ONE,
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
            .ingest_us_stock_bars(&harness.source, now)
            .await
            .expect("US stock bar ingestion succeeds");
        let calls = harness.source.calls.lock().await.clone();
        let daily_bars = harness.repository.bars.lock().await.clone();
        let minute_bars = harness.repository.minute_bars.lock().await.clone();
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
            vec![
                bar("US:QZ-7", Timeframe::Daily, date("2040-01-01T00:00:00Z")),
                bar("US:QZ7", Timeframe::Daily, now),
                bar("US:QZ-7", Timeframe::Daily, now),
            ],
            vec![
                bar("US:QZ-7", Timeframe::Minute, date("2040-01-02T11:59:00Z")),
                bar("US:QZ7", Timeframe::Minute, now),
                bar("US:QZ-7", Timeframe::Minute, now),
            ],
        );

        assert_eq!((stats, calls, daily_bars, minute_bars), expected);
    }

    #[rstest]
    #[tokio::test]
    async fn groups_existing_symbols_by_their_own_latest_bar(harness: Harness) {
        let instrument_ids = (0..101)
            .map(|index| format!("US:QZ{index:03}"))
            .collect::<Vec<_>>();
        let recent_bar = date("2040-01-01T00:00:00Z");
        let older_bar = date("2039-01-01T00:00:00Z");
        let saved_bars = instrument_ids
            .iter()
            .enumerate()
            .flat_map(|(index, instrument_id)| {
                let timestamp = if index == 0 { older_bar } else { recent_bar };
                [
                    bar(instrument_id, Timeframe::Daily, timestamp),
                    bar(instrument_id, Timeframe::Minute, timestamp),
                ]
            })
            .collect::<Vec<_>>();
        harness
            .repository
            .seed_us_stock_targets(instrument_ids.clone())
            .await;
        harness.repository.seed_bars(saved_bars).await;
        let now = Utc
            .with_ymd_and_hms(2040, 1, 2, 12, 0, 0)
            .single()
            .unwrap_or_default();
        harness
            .use_cases
            .ingest_us_stock_bars(&harness.source, now)
            .await
            .expect("US stock bar ingestion succeeds");

        let actual = harness.source.calls.lock().await.clone();
        let expected = vec![
            UsStockBarQuery {
                instrument_ids: instrument_ids[1..].to_vec(),
                timeframe: Timeframe::Daily,
                from: recent_bar,
                to: now,
                page_token: None,
            },
            UsStockBarQuery {
                instrument_ids: vec![instrument_ids[0].clone()],
                timeframe: Timeframe::Daily,
                from: older_bar,
                to: now,
                page_token: None,
            },
            UsStockBarQuery {
                instrument_ids: instrument_ids[1..].to_vec(),
                timeframe: Timeframe::Minute,
                from: recent_bar,
                to: now,
                page_token: None,
            },
            UsStockBarQuery {
                instrument_ids: vec![instrument_ids[0].clone()],
                timeframe: Timeframe::Minute,
                from: older_bar,
                to: now,
                page_token: None,
            },
        ];

        assert_eq!(actual, expected);
    }

    #[rstest]
    #[tokio::test]
    async fn fails_when_the_page_token_does_not_advance(harness: Harness) {
        harness
            .repository
            .seed_us_stock_targets(vec!["US:QZ7".to_owned()])
            .await;
        let mut harness = harness;
        harness.source.repeat_page_token = true;
        let now = Utc
            .with_ymd_and_hms(2040, 1, 2, 12, 0, 0)
            .single()
            .unwrap_or_default();

        let result = harness
            .use_cases
            .ingest_us_stock_bars(&harness.source, now)
            .await
            .map_err(|error| error.to_string());
        let calls = harness.source.calls.lock().await.clone();

        assert_eq!(
            (result, calls),
            (
                Err("US stock bar source error: pagination token did not advance".to_owned()),
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
                ],
            ),
        );
    }
}
