use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use chrono::NaiveDate;
use core_domain::IndicatorObservation;
use rstest::rstest;
use rust_decimal::Decimal;
use uuid::Uuid;

use crate::indicator_observation_batch_source::{
    IndicatorObservationBatch, IndicatorObservationBatchSource,
};
use crate::indicator_observation_source::{
    IndicatorObservationSource, IndicatorObservationSourceError,
};
use crate::persistence::PersistenceError;
use crate::strategy_scope::{StrategyScope, StrategyScopeSource, StrategyScopeSourceError};

use super::types::{
    IndicatorObservationIngestSeriesResult, IndicatorObservationMetadata,
    IndicatorObservationQuery, IndicatorObservationSeriesDefinition,
};
use super::{
    FakeIndicatorObservationRepository, IndicatorObservationBatchError,
    IndicatorObservationRepository, IndicatorObservationRepositoryError,
    IndicatorObservationUseCases,
};

#[derive(Default)]
struct FakeSource {
    requests: Mutex<Vec<(String, Option<NaiveDate>)>>,
    responses: Mutex<HashMap<usize, Vec<IndicatorObservation>>>,
    fail_call: Option<usize>,
}

#[derive(Default)]
struct FakeBatchSource {
    requests: Mutex<Vec<(Vec<String>, NaiveDate, NaiveDate)>>,
    observations: HashMap<String, Vec<IndicatorObservation>>,
    errors: Vec<IndicatorObservationBatchError>,
}

#[async_trait]
impl IndicatorObservationBatchSource for FakeBatchSource {
    async fn fetch_observations(
        &self,
        series_ids: &[&str],
        from: NaiveDate,
        to: NaiveDate,
    ) -> Result<IndicatorObservationBatch, IndicatorObservationSourceError> {
        self.requests.lock().expect("lock requests").push((
            series_ids
                .iter()
                .map(|series_id| (*series_id).to_string())
                .collect(),
            from,
            to,
        ));
        Ok(IndicatorObservationBatch {
            observations: self.observations.clone(),
            errors: self.errors.clone(),
        })
    }
}

struct FailingUpsertRepository {
    inner: FakeIndicatorObservationRepository,
    failing_indicator_id: &'static str,
}

#[async_trait]
impl IndicatorObservationRepository for FailingUpsertRepository {
    async fn ensure_indicator(
        &self,
        metadata: IndicatorObservationMetadata,
    ) -> Result<(), IndicatorObservationRepositoryError> {
        self.inner.ensure_indicator(metadata).await
    }

    async fn find_latest_date(
        &self,
        indicator_id: &str,
    ) -> Result<Option<NaiveDate>, IndicatorObservationRepositoryError> {
        self.inner.find_latest_date(indicator_id).await
    }

    async fn upsert_observations(
        &self,
        indicator_id: &str,
        observations: Vec<IndicatorObservation>,
    ) -> Result<usize, IndicatorObservationRepositoryError> {
        if indicator_id == self.failing_indicator_id {
            return Err(PersistenceError::Database("synthetic upsert failure".to_string()).into());
        }
        self.inner
            .upsert_observations(indicator_id, observations)
            .await
    }

    async fn find_observations(
        &self,
        query: IndicatorObservationQuery,
    ) -> Result<Vec<IndicatorObservation>, IndicatorObservationRepositoryError> {
        self.inner.find_observations(query).await
    }
}

impl FakeSource {
    fn with_response(call: usize, observations: Vec<IndicatorObservation>) -> Self {
        Self {
            responses: Mutex::new(HashMap::from([(call, observations)])),
            ..Self::default()
        }
    }

    fn failing_on(call: usize) -> Self {
        Self {
            fail_call: Some(call),
            ..Self::default()
        }
    }
}

#[async_trait]
impl IndicatorObservationSource for FakeSource {
    async fn fetch_observations(
        &self,
        series_id: &str,
        observation_start: Option<NaiveDate>,
    ) -> Result<Vec<IndicatorObservation>, IndicatorObservationSourceError> {
        let mut requests = self.requests.lock().expect("lock requests");
        let call = requests.len();
        requests.push((series_id.to_string(), observation_start));
        drop(requests);

        if self.fail_call == Some(call) {
            return Err(IndicatorObservationSourceError::Api {
                status: 503,
                message: "test source failure".to_string(),
            });
        }
        Ok(self
            .responses
            .lock()
            .expect("lock responses")
            .remove(&call)
            .unwrap_or_default())
    }
}

#[derive(Default)]
struct FakeScopeSource {
    existing_ids: HashSet<Uuid>,
}

#[async_trait]
impl StrategyScopeSource for FakeScopeSource {
    async fn existing_ids(&self, _ids: &[Uuid]) -> Result<HashSet<Uuid>, StrategyScopeSourceError> {
        Ok(self.existing_ids.clone())
    }
}

async fn existing_strategy_scope() -> StrategyScope {
    let id = Uuid::new_v4();
    let source = FakeScopeSource {
        existing_ids: HashSet::from([id]),
    };
    StrategyScope::verify(id, &source)
        .await
        .expect("strategy exists")
}

fn date(day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 9, day).expect("valid date")
}

fn observation(day: u32, value: &str) -> IndicatorObservation {
    IndicatorObservation {
        date: date(day),
        value: Decimal::from_str_exact(value).expect("valid decimal"),
    }
}

const BATCH_DEFINITIONS: [IndicatorObservationSeriesDefinition; 2] = [
    IndicatorObservationSeriesDefinition {
        series_id: "SERIES_ALPHA",
        indicator_id: "INDICATOR_ALPHA",
        name: "架空指数 A",
        kind: "index",
    },
    IndicatorObservationSeriesDefinition {
        series_id: "SERIES_BETA",
        indicator_id: "INDICATOR_BETA",
        name: "架空指数 B",
        kind: "index",
    },
];

fn use_cases(repository: Arc<dyn IndicatorObservationRepository>) -> IndicatorObservationUseCases {
    IndicatorObservationUseCases::new(repository)
}

#[tokio::test]
async fn read_returns_trimmed_identifier_and_inclusive_observations_in_date_order() {
    let strategy_scope = existing_strategy_scope().await;
    let repository = Arc::new(FakeIndicatorObservationRepository::new());
    repository
        .seed_observation("INDICATOR_TEST", date(10), Decimal::from(10))
        .await;
    repository
        .seed_observation("INDICATOR_TEST", date(3), Decimal::from(3))
        .await;
    repository
        .seed_observation("INDICATOR_TEST", date(2), Decimal::from(2))
        .await;

    let result = use_cases(repository)
        .read(
            strategy_scope,
            IndicatorObservationQuery {
                indicator_id: "  INDICATOR_TEST ".to_string(),
                from: date(3),
                to: date(10),
            },
        )
        .await
        .expect("read succeeds");

    assert_eq!(
        result,
        super::types::IndicatorObservationReadResult {
            indicator_id: "INDICATOR_TEST".to_string(),
            observations: vec![observation(3, "3"), observation(10, "10")],
        },
    );
}

#[rstest]
#[case::empty_identifier(
    IndicatorObservationQuery {
        indicator_id: "  ".to_string(),
        from: date(1),
        to: date(2),
    },
    "indicator_id must not be empty",
)]
#[case::reversed_range(
    IndicatorObservationQuery {
        indicator_id: "INDICATOR_TEST".to_string(),
        from: date(2),
        to: date(1),
    },
    "from must be on or before to",
)]
#[tokio::test]
async fn read_rejects_invalid_queries(
    #[case] query: IndicatorObservationQuery,
    #[case] message: &str,
) {
    let strategy_scope = existing_strategy_scope().await;
    let result = use_cases(Arc::new(FakeIndicatorObservationRepository::new()))
        .read(strategy_scope, query)
        .await;

    assert_eq!(
        result.map_err(|error| error.to_string()),
        Err(message.to_string()),
    );
}

#[tokio::test]
async fn ingest_uses_lookback_updates_values_and_preserves_existing_metadata() {
    let repository = Arc::new(FakeIndicatorObservationRepository::new());
    repository
        .indicators
        .lock()
        .expect("lock indicators")
        .insert(
            "USDJPY".to_string(),
            IndicatorObservationMetadata {
                indicator_id: "USDJPY".to_string(),
                name: "既存名".to_string(),
                kind: "custom".to_string(),
            },
        );
    repository
        .seed_observation("USDJPY", date(1), Decimal::from(1))
        .await;
    let source = FakeSource::with_response(0, vec![observation(1, "2")]);

    let result = use_cases(repository.clone()).ingest(&source).await;
    let requests = source.requests.lock().expect("lock requests").clone();
    let stored = repository
        .observations
        .lock()
        .expect("lock observations")
        .get(&("USDJPY".to_string(), date(1)))
        .copied();
    let metadata = repository
        .indicators
        .lock()
        .expect("lock indicators")
        .get("USDJPY")
        .cloned();
    let mut indicators: Vec<_> = repository
        .indicators
        .lock()
        .expect("lock indicators")
        .values()
        .cloned()
        .collect();
    indicators.sort_by_key(|indicator| indicator.indicator_id.clone());
    let normalized: Vec<_> = result
        .series
        .iter()
        .map(|outcome| match outcome {
            IndicatorObservationIngestSeriesResult::Succeeded { upserted, .. } => (true, *upserted),
            IndicatorObservationIngestSeriesResult::Failed { .. } => (false, 0),
        })
        .collect();

    assert_eq!(
        (normalized, requests, stored, metadata, indicators),
        (
            vec![
                (true, 1),
                (true, 0),
                (true, 0),
                (true, 0),
                (true, 0),
                (true, 0),
                (true, 0),
            ],
            vec![
                (
                    "DEXJPUS".to_string(),
                    Some(date(1) - chrono::Duration::days(10))
                ),
                ("VIXCLS".to_string(), None),
                ("DGS10".to_string(), None),
                ("NIKKEI225".to_string(), None),
                ("SP500".to_string(), None),
                ("NASDAQCOM".to_string(), None),
                ("NASDAQSOX".to_string(), None),
            ],
            Some(Decimal::from(2)),
            Some(IndicatorObservationMetadata {
                indicator_id: "USDJPY".to_string(),
                name: "既存名".to_string(),
                kind: "custom".to_string(),
            }),
            vec![
                IndicatorObservationMetadata {
                    indicator_id: "NASDAQ".to_string(),
                    name: "NASDAQ総合指数".to_string(),
                    kind: "index".to_string(),
                },
                IndicatorObservationMetadata {
                    indicator_id: "NIKKEI225".to_string(),
                    name: "日経225".to_string(),
                    kind: "index".to_string(),
                },
                IndicatorObservationMetadata {
                    indicator_id: "SOX".to_string(),
                    name: "フィラデルフィア半導体株指数".to_string(),
                    kind: "index".to_string(),
                },
                IndicatorObservationMetadata {
                    indicator_id: "SP500".to_string(),
                    name: "S&P 500".to_string(),
                    kind: "index".to_string(),
                },
                IndicatorObservationMetadata {
                    indicator_id: "US10Y".to_string(),
                    name: "米10年債利回り".to_string(),
                    kind: "rate".to_string(),
                },
                IndicatorObservationMetadata {
                    indicator_id: "USDJPY".to_string(),
                    name: "既存名".to_string(),
                    kind: "custom".to_string(),
                },
                IndicatorObservationMetadata {
                    indicator_id: "VIX".to_string(),
                    name: "VIX".to_string(),
                    kind: "volatility".to_string(),
                },
            ],
        ),
    );
}

#[tokio::test]
async fn ingest_continues_with_later_series_after_a_source_failure() {
    let source = FakeSource::failing_on(0);
    let result = use_cases(Arc::new(FakeIndicatorObservationRepository::new()))
        .ingest(&source)
        .await;
    let starts: Vec<_> = source
        .requests
        .lock()
        .expect("lock requests")
        .iter()
        .map(|(_, start)| *start)
        .collect();
    let normalized: Vec<_> = result
        .series
        .iter()
        .map(|outcome| match outcome {
            IndicatorObservationIngestSeriesResult::Succeeded { upserted, .. } => (true, *upserted),
            IndicatorObservationIngestSeriesResult::Failed { .. } => (false, 0),
        })
        .collect();

    assert_eq!(
        (normalized, starts),
        (
            vec![
                (false, 0),
                (true, 0),
                (true, 0),
                (true, 0),
                (true, 0),
                (true, 0),
                (true, 0),
            ],
            vec![None; 7],
        ),
    );
}

#[tokio::test]
async fn ingest_batch_fetches_and_persists_each_configured_series_for_the_requested_period() {
    let repository = Arc::new(FakeIndicatorObservationRepository::new());
    let source = FakeBatchSource {
        observations: HashMap::from([
            ("SERIES_ALPHA".to_string(), vec![observation(2, "12.5")]),
            ("SERIES_BETA".to_string(), vec![observation(3, "8.75")]),
        ]),
        ..FakeBatchSource::default()
    };

    let result = use_cases(repository.clone())
        .ingest_batch(&source, &BATCH_DEFINITIONS, Some(date(1)), date(5))
        .await;
    let requests = source.requests.lock().expect("lock requests").clone();
    let indicators = repository
        .indicators
        .lock()
        .expect("lock indicators")
        .clone();
    let observations = repository
        .observations
        .lock()
        .expect("lock observations")
        .clone();

    assert_eq!(
        (result, requests, indicators, observations),
        (
            super::types::IndicatorObservationIngestResult {
                series: vec![
                    IndicatorObservationIngestSeriesResult::Succeeded {
                        series_id: "SERIES_ALPHA".to_string(),
                        upserted: 1,
                    },
                    IndicatorObservationIngestSeriesResult::Succeeded {
                        series_id: "SERIES_BETA".to_string(),
                        upserted: 1,
                    },
                ],
                errors: Vec::new(),
            },
            vec![(
                vec!["SERIES_ALPHA".to_string(), "SERIES_BETA".to_string()],
                date(1),
                date(5),
            )],
            HashMap::from([
                (
                    "INDICATOR_ALPHA".to_string(),
                    IndicatorObservationMetadata {
                        indicator_id: "INDICATOR_ALPHA".to_string(),
                        name: "架空指数 A".to_string(),
                        kind: "index".to_string(),
                    },
                ),
                (
                    "INDICATOR_BETA".to_string(),
                    IndicatorObservationMetadata {
                        indicator_id: "INDICATOR_BETA".to_string(),
                        name: "架空指数 B".to_string(),
                        kind: "index".to_string(),
                    },
                ),
            ]),
            BTreeMap::from([
                (
                    ("INDICATOR_ALPHA".to_string(), date(2)),
                    Decimal::from_str_exact("12.5").expect("valid decimal")
                ),
                (
                    ("INDICATOR_BETA".to_string(), date(3)),
                    Decimal::from_str_exact("8.75").expect("valid decimal")
                ),
            ]),
        ),
    );
}

#[tokio::test]
async fn ingest_batch_persists_available_dates_and_reports_fetch_failures() {
    let repository = Arc::new(FakeIndicatorObservationRepository::new());
    let source = FakeBatchSource {
        observations: HashMap::from([
            ("SERIES_ALPHA".to_string(), vec![observation(2, "12.5")]),
            ("SERIES_BETA".to_string(), vec![observation(2, "8.75")]),
        ]),
        errors: vec![IndicatorObservationBatchError {
            date: date(1),
            message: "network error: synthetic failure".to_string(),
        }],
        ..FakeBatchSource::default()
    };

    let result = use_cases(repository.clone())
        .ingest_batch(&source, &BATCH_DEFINITIONS, Some(date(1)), date(5))
        .await;
    let observations = repository
        .observations
        .lock()
        .expect("lock observations")
        .clone();

    assert_eq!(
        (result, observations),
        (
            super::types::IndicatorObservationIngestResult {
                series: vec![
                    IndicatorObservationIngestSeriesResult::Succeeded {
                        series_id: "SERIES_ALPHA".to_string(),
                        upserted: 1,
                    },
                    IndicatorObservationIngestSeriesResult::Succeeded {
                        series_id: "SERIES_BETA".to_string(),
                        upserted: 1,
                    },
                ],
                errors: vec![IndicatorObservationBatchError {
                    date: date(1),
                    message: "network error: synthetic failure".to_string(),
                }],
            },
            BTreeMap::from([
                (
                    ("INDICATOR_ALPHA".to_string(), date(2)),
                    Decimal::from_str_exact("12.5").expect("valid decimal")
                ),
                (
                    ("INDICATOR_BETA".to_string(), date(2)),
                    Decimal::from_str_exact("8.75").expect("valid decimal")
                ),
            ]),
        ),
    );
}

#[tokio::test]
async fn ingest_batch_continues_after_a_series_upsert_fails() {
    let repository = Arc::new(FailingUpsertRepository {
        inner: FakeIndicatorObservationRepository::new(),
        failing_indicator_id: "INDICATOR_ALPHA",
    });
    let source = FakeBatchSource {
        observations: HashMap::from([
            ("SERIES_ALPHA".to_string(), vec![observation(2, "12.5")]),
            ("SERIES_BETA".to_string(), vec![observation(3, "8.75")]),
        ]),
        ..FakeBatchSource::default()
    };

    let result = use_cases(repository.clone())
        .ingest_batch(&source, &BATCH_DEFINITIONS, Some(date(1)), date(5))
        .await;
    let observations = repository
        .inner
        .observations
        .lock()
        .expect("lock observations")
        .clone();

    assert_eq!(
        (result, observations),
        (
            super::types::IndicatorObservationIngestResult {
                series: vec![
                    IndicatorObservationIngestSeriesResult::Failed {
                        series_id: "SERIES_ALPHA".to_string(),
                        error: "synthetic upsert failure".to_string(),
                    },
                    IndicatorObservationIngestSeriesResult::Succeeded {
                        series_id: "SERIES_BETA".to_string(),
                        upserted: 1,
                    },
                ],
                errors: Vec::new(),
            },
            BTreeMap::from([(
                ("INDICATOR_BETA".to_string(), date(3)),
                Decimal::from_str_exact("8.75").expect("valid decimal")
            )]),
        ),
    );
}

#[tokio::test]
async fn ingest_batch_uses_the_earliest_series_lookback_when_no_range_is_given() {
    let repository = Arc::new(FakeIndicatorObservationRepository::new());
    repository
        .seed_observation("INDICATOR_ALPHA", date(20), Decimal::from(20))
        .await;
    repository
        .seed_observation("INDICATOR_BETA", date(25), Decimal::from(25))
        .await;
    let source = FakeBatchSource {
        observations: HashMap::from([
            ("SERIES_ALPHA".to_string(), Vec::new()),
            ("SERIES_BETA".to_string(), Vec::new()),
        ]),
        ..FakeBatchSource::default()
    };

    let result = use_cases(repository)
        .ingest_batch(&source, &BATCH_DEFINITIONS, None, date(30))
        .await;
    let requests = source.requests.lock().expect("lock requests").clone();

    assert_eq!(
        (result, requests),
        (
            super::types::IndicatorObservationIngestResult {
                series: vec![
                    IndicatorObservationIngestSeriesResult::Succeeded {
                        series_id: "SERIES_ALPHA".to_string(),
                        upserted: 0,
                    },
                    IndicatorObservationIngestSeriesResult::Succeeded {
                        series_id: "SERIES_BETA".to_string(),
                        upserted: 0,
                    },
                ],
                errors: Vec::new(),
            },
            vec![(
                vec!["SERIES_ALPHA".to_string(), "SERIES_BETA".to_string()],
                date(10),
                date(30),
            )],
        ),
    );
}

#[tokio::test]
async fn ingest_batch_uses_the_default_lookback_when_no_series_has_observations() {
    let repository = Arc::new(FakeIndicatorObservationRepository::new());
    let source = FakeBatchSource {
        observations: HashMap::from([
            ("SERIES_ALPHA".to_string(), Vec::new()),
            ("SERIES_BETA".to_string(), Vec::new()),
        ]),
        ..FakeBatchSource::default()
    };

    let result = use_cases(repository)
        .ingest_batch(&source, &BATCH_DEFINITIONS, None, date(30))
        .await;
    let requests = source.requests.lock().expect("lock requests").clone();

    assert_eq!(
        (result, requests),
        (
            super::types::IndicatorObservationIngestResult {
                series: vec![
                    IndicatorObservationIngestSeriesResult::Succeeded {
                        series_id: "SERIES_ALPHA".to_string(),
                        upserted: 0,
                    },
                    IndicatorObservationIngestSeriesResult::Succeeded {
                        series_id: "SERIES_BETA".to_string(),
                        upserted: 0,
                    },
                ],
                errors: Vec::new(),
            },
            vec![(
                vec!["SERIES_ALPHA".to_string(), "SERIES_BETA".to_string()],
                date(20),
                date(30),
            )],
        ),
    );
}

#[tokio::test]
async fn ingest_eoddata_series_maps_ksic_to_kospi_and_persists_observations() {
    let repository = Arc::new(FakeIndicatorObservationRepository::new());
    let source = FakeSource::with_response(0, vec![observation(3, "1234.56")]);

    let result = use_cases(repository.clone())
        .ingest_eoddata_series(&source)
        .await;
    let requests = source.requests.lock().expect("lock requests").clone();
    let stored = repository
        .observations
        .lock()
        .expect("lock observations")
        .get(&("KOSPI".to_string(), date(3)))
        .copied();
    let registered = repository
        .indicators
        .lock()
        .expect("lock indicators")
        .get("KOSPI")
        .cloned();

    assert_eq!(
        (result, requests, stored, registered),
        (
            Ok(1),
            vec![("KSIC".to_string(), None)],
            Some(Decimal::from_str_exact("1234.56").expect("valid decimal")),
            Some(IndicatorObservationMetadata {
                indicator_id: "KOSPI".to_string(),
                name: "韓国総合株価指数".to_string(),
                kind: "index".to_string(),
            }),
        ),
    );
}
