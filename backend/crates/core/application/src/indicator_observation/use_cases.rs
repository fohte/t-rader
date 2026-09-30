use chrono::Duration;
use core_domain::IndicatorObservation;

use crate::indicator_observation_source::IndicatorObservationSource;
use crate::strategy_scope::StrategyScope;

use super::error::IndicatorObservationUseCaseError;
use super::repository::SharedIndicatorObservationRepository;
use super::types::{
    IndicatorObservationIngestResult, IndicatorObservationIngestSeriesResult,
    IndicatorObservationMetadata, IndicatorObservationQuery, IndicatorObservationReadResult,
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
    ) -> Result<usize, IndicatorObservationUseCaseError> {
        let metadata = IndicatorObservationMetadata {
            indicator_id: definition.indicator_id.to_string(),
            name: definition.name.to_string(),
            kind: definition.kind.to_string(),
        };
        self.repository.ensure_indicator(metadata.clone()).await?;

        let latest = self
            .repository
            .find_latest_date(&metadata.indicator_id)
            .await?;
        let observation_start = latest.map(|date| date - Duration::days(LOOKBACK_DAYS));
        let observations: Vec<IndicatorObservation> = source
            .fetch_observations(definition.series_id, observation_start)
            .await?;

        self.repository
            .upsert_observations(&metadata.indicator_id, observations)
            .await
            .map_err(Into::into)
    }
}
