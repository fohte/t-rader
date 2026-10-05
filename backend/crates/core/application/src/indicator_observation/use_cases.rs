use chrono::Duration;
use chrono::NaiveDate;
use core_domain::IndicatorObservation;

use crate::indicator_observation_batch_source::IndicatorObservationBatchSource;
use crate::indicator_observation_source::IndicatorObservationSource;
use crate::strategy_scope::StrategyScope;

use super::error::IndicatorObservationUseCaseError;
use super::repository::SharedIndicatorObservationRepository;
use super::types::{
    IndicatorObservationIngestResult, IndicatorObservationIngestSeriesResult,
    IndicatorObservationMetadata, IndicatorObservationQuery, IndicatorObservationReadResult,
    IndicatorObservationSeriesDefinition,
};

const LOOKBACK_DAYS: i64 = 10;

struct SeriesDefinition {
    series_id: &'static str,
    indicator_id: &'static str,
    name: &'static str,
    kind: &'static str,
}

const SERIES: &[SeriesDefinition] = &[
    SeriesDefinition {
        series_id: "DEXJPUS",
        indicator_id: "USDJPY",
        name: "ドル円",
        kind: "fx",
    },
    SeriesDefinition {
        series_id: "VIXCLS",
        indicator_id: "VIX",
        name: "VIX",
        kind: "volatility",
    },
    SeriesDefinition {
        series_id: "DGS10",
        indicator_id: "US10Y",
        name: "米10年債利回り",
        kind: "rate",
    },
    SeriesDefinition {
        series_id: "NIKKEI225",
        indicator_id: "NIKKEI225",
        name: "日経225",
        kind: "index",
    },
];

pub const TWSE_SERIES: &[IndicatorObservationSeriesDefinition] = &[
    IndicatorObservationSeriesDefinition {
        series_id: "TAIEX",
        indicator_id: "TAIEX",
        name: "台湾加権指数",
        kind: "index",
    },
    IndicatorObservationSeriesDefinition {
        series_id: "TW_SEMI",
        indicator_id: "TW_SEMI",
        name: "台湾半導体指数",
        kind: "index",
    },
];

#[derive(Clone)]
pub struct IndicatorObservationUseCases {
    repository: SharedIndicatorObservationRepository,
}

impl IndicatorObservationUseCases {
    pub fn new(repository: SharedIndicatorObservationRepository) -> Self {
        Self { repository }
    }

    pub async fn ingest(
        &self,
        source: &dyn IndicatorObservationSource,
    ) -> IndicatorObservationIngestResult {
        let mut results = Vec::with_capacity(SERIES.len());
        for definition in SERIES {
            let result = self.ingest_series(source, definition).await;
            results.push(match result {
                Ok(upserted) => IndicatorObservationIngestSeriesResult::Succeeded {
                    series_id: definition.series_id.to_string(),
                    upserted,
                },
                Err(error) => IndicatorObservationIngestSeriesResult::Failed {
                    series_id: definition.series_id.to_string(),
                    error: error.to_string(),
                },
            });
        }
        IndicatorObservationIngestResult { series: results }
    }

    pub async fn ingest_batch(
        &self,
        source: &dyn IndicatorObservationBatchSource,
        definitions: &[IndicatorObservationSeriesDefinition],
        observation_start: Option<NaiveDate>,
        observation_end: NaiveDate,
    ) -> IndicatorObservationIngestResult {
        let mut prepared = Vec::with_capacity(definitions.len());
        let mut results = Vec::with_capacity(definitions.len());
        let mut starts = Vec::with_capacity(definitions.len());

        for definition in definitions {
            let metadata = IndicatorObservationMetadata {
                indicator_id: definition.indicator_id.to_string(),
                name: definition.name.to_string(),
                kind: definition.kind.to_string(),
            };
            if let Err(error) = self.repository.ensure_indicator(metadata.clone()).await {
                results.push(IndicatorObservationIngestSeriesResult::Failed {
                    series_id: definition.series_id.to_string(),
                    error: error.to_string(),
                });
                continue;
            }

            let start = if let Some(start) = observation_start {
                start
            } else {
                match self
                    .repository
                    .find_latest_date(&metadata.indicator_id)
                    .await
                {
                    Ok(latest) => latest
                        .map(|date| date - Duration::days(LOOKBACK_DAYS))
                        .unwrap_or(observation_end - Duration::days(LOOKBACK_DAYS)),
                    Err(error) => {
                        results.push(IndicatorObservationIngestSeriesResult::Failed {
                            series_id: definition.series_id.to_string(),
                            error: error.to_string(),
                        });
                        continue;
                    }
                }
            };
            starts.push(start);
            prepared.push((definition, metadata));
        }

        if prepared.is_empty() {
            return IndicatorObservationIngestResult { series: results };
        }

        let start = starts.into_iter().min().unwrap_or(observation_end);
        if start > observation_end {
            let error = "from must be on or before to".to_string();
            results.extend(prepared.into_iter().map(|(definition, _)| {
                IndicatorObservationIngestSeriesResult::Failed {
                    series_id: definition.series_id.to_string(),
                    error: error.clone(),
                }
            }));
            return IndicatorObservationIngestResult { series: results };
        }

        let series_ids: Vec<_> = prepared
            .iter()
            .map(|(definition, _)| definition.series_id)
            .collect();
        let observations = match source
            .fetch_observations(&series_ids, start, observation_end)
            .await
        {
            Ok(observations) => observations,
            Err(error) => {
                results.extend(prepared.into_iter().map(|(definition, _)| {
                    IndicatorObservationIngestSeriesResult::Failed {
                        series_id: definition.series_id.to_string(),
                        error: error.to_string(),
                    }
                }));
                return IndicatorObservationIngestResult { series: results };
            }
        };

        for (definition, metadata) in prepared {
            let Some(series_observations) = observations.get(definition.series_id) else {
                results.push(IndicatorObservationIngestSeriesResult::Failed {
                    series_id: definition.series_id.to_string(),
                    error: "source response is missing the requested series".to_string(),
                });
                continue;
            };
            match self
                .repository
                .upsert_observations(&metadata.indicator_id, series_observations.clone())
                .await
            {
                Ok(upserted) => results.push(IndicatorObservationIngestSeriesResult::Succeeded {
                    series_id: definition.series_id.to_string(),
                    upserted,
                }),
                Err(error) => results.push(IndicatorObservationIngestSeriesResult::Failed {
                    series_id: definition.series_id.to_string(),
                    error: error.to_string(),
                }),
            }
        }
        IndicatorObservationIngestResult { series: results }
    }

    pub async fn read(
        &self,
        _scope: StrategyScope,
        query: IndicatorObservationQuery,
    ) -> Result<IndicatorObservationReadResult, IndicatorObservationUseCaseError> {
        let indicator_id = query.indicator_id.trim().to_string();
        if indicator_id.is_empty() {
            return Err(IndicatorObservationUseCaseError::Validation(
                "indicator_id must not be empty".to_string(),
            ));
        }
        if query.from > query.to {
            return Err(IndicatorObservationUseCaseError::Validation(
                "from must be on or before to".to_string(),
            ));
        }

        let observations = self
            .repository
            .find_observations(IndicatorObservationQuery {
                indicator_id: indicator_id.clone(),
                from: query.from,
                to: query.to,
            })
            .await?;

        Ok(IndicatorObservationReadResult {
            indicator_id,
            observations,
        })
    }

    async fn ingest_series(
        &self,
        source: &dyn IndicatorObservationSource,
        definition: &SeriesDefinition,
    ) -> Result<usize, String> {
        let metadata = IndicatorObservationMetadata {
            indicator_id: definition.indicator_id.to_string(),
            name: definition.name.to_string(),
            kind: definition.kind.to_string(),
        };
        self.repository
            .ensure_indicator(metadata.clone())
            .await
            .map_err(|error| error.to_string())?;

        let latest = self
            .repository
            .find_latest_date(&metadata.indicator_id)
            .await
            .map_err(|error| error.to_string())?;
        let observation_start = latest.map(|date| date - Duration::days(LOOKBACK_DAYS));
        let observations: Vec<IndicatorObservation> = source
            .fetch_observations(definition.series_id, observation_start)
            .await
            .map_err(|error| error.to_string())?;

        self.repository
            .upsert_observations(&metadata.indicator_id, observations)
            .await
            .map_err(|error| error.to_string())
    }
}
