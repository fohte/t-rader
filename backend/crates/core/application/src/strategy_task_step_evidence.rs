use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, FixedOffset, NaiveDate, Utc};
use core_domain::note_price_reference::PriceReferenceField;
use serde::{Deserialize, Serialize};
use serde_json::json;
use thiserror::Error;
use uuid::Uuid;

use crate::persistence::PersistenceError;
use crate::unit_of_work::UnitOfWorkTransaction;

/// evidence snapshot の肥大化を防ぐため、保存するバー数を制限する。
const MAX_SNAPSHOT_BARS: usize = 5_000;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct QueryDataBar {
    pub timestamp: DateTime<FixedOffset>,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StrategyTaskStepEvidence {
    pub id: Uuid,
    /// strategy_task_step の行は照合 job が非同期反映するため、外部キーではなく相関用 UUID とする。
    pub execution_step_id: Uuid,
    pub source: String,
    pub source_ref: String,
    pub observed_at: DateTime<FixedOffset>,
    pub published_at: Option<DateTime<FixedOffset>>,
    pub effective_at: Option<DateTime<FixedOffset>>,
    pub snapshot: serde_json::Value,
}

impl StrategyTaskStepEvidence {
    pub fn query_data_bar_value(
        &self,
        date: NaiveDate,
        field: PriceReferenceField,
    ) -> Option<serde_json::Value> {
        let snapshot: QueryDataSnapshotBars = serde_json::from_value(self.snapshot.clone()).ok()?;
        let bar = snapshot
            .bars
            .iter()
            .find(|bar| bar.timestamp.date_naive() == date)?;

        match field {
            PriceReferenceField::Open => serde_json::Number::from_f64(bar.open),
            PriceReferenceField::High => serde_json::Number::from_f64(bar.high),
            PriceReferenceField::Low => serde_json::Number::from_f64(bar.low),
            PriceReferenceField::Close => serde_json::Number::from_f64(bar.close),
            PriceReferenceField::Volume => Some(bar.volume.into()),
        }
        .map(serde_json::Value::Number)
    }
}

#[derive(Deserialize)]
struct QueryDataSnapshotBars {
    bars: Vec<QueryDataBar>,
}

#[derive(Debug, Error)]
pub enum StrategyTaskStepEvidenceRepositoryError {
    #[error(transparent)]
    Database(#[from] PersistenceError),
    #[error("transaction has an unexpected type")]
    InvalidTransaction,
}

#[async_trait]
pub trait StrategyTaskStepEvidenceRepository: Send + Sync {
    async fn insert(
        &self,
        evidence: StrategyTaskStepEvidence,
    ) -> Result<(), StrategyTaskStepEvidenceRepositoryError>;
    async fn find_query_data(
        &self,
        transaction: &UnitOfWorkTransaction,
        execution_step_id: Uuid,
        instrument_id: &str,
    ) -> Result<Vec<StrategyTaskStepEvidence>, StrategyTaskStepEvidenceRepositoryError>;
    async fn find_query_data_by_evidence_id(
        &self,
        transaction: &UnitOfWorkTransaction,
        evidence_id: Uuid,
    ) -> Result<Option<StrategyTaskStepEvidence>, StrategyTaskStepEvidenceRepositoryError>;
}

#[cfg(test)]
#[derive(Clone, Default)]
pub(crate) struct FakeStrategyTaskStepEvidenceRepository {
    records: std::sync::Arc<std::sync::Mutex<Vec<StrategyTaskStepEvidence>>>,
}

#[cfg(test)]
impl FakeStrategyTaskStepEvidenceRepository {
    pub(crate) fn new(records: Vec<StrategyTaskStepEvidence>) -> Self {
        Self {
            records: std::sync::Arc::new(std::sync::Mutex::new(records)),
        }
    }

    fn with_records<T>(
        &self,
        operation: impl FnOnce(&mut Vec<StrategyTaskStepEvidence>) -> T,
    ) -> T {
        let mut records = self
            .records
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        operation(&mut records)
    }
}

#[cfg(test)]
#[async_trait]
impl StrategyTaskStepEvidenceRepository for FakeStrategyTaskStepEvidenceRepository {
    async fn insert(
        &self,
        evidence: StrategyTaskStepEvidence,
    ) -> Result<(), StrategyTaskStepEvidenceRepositoryError> {
        self.with_records(|records| records.push(evidence));
        Ok(())
    }

    async fn find_query_data(
        &self,
        _transaction: &UnitOfWorkTransaction,
        execution_step_id: Uuid,
        instrument_id: &str,
    ) -> Result<Vec<StrategyTaskStepEvidence>, StrategyTaskStepEvidenceRepositoryError> {
        Ok(self.with_records(|records| {
            records
                .iter()
                .filter(|evidence| {
                    evidence.execution_step_id == execution_step_id
                        && evidence.source == "query_data"
                        && evidence.source_ref == instrument_id
                })
                .cloned()
                .collect()
        }))
    }

    async fn find_query_data_by_evidence_id(
        &self,
        _transaction: &UnitOfWorkTransaction,
        evidence_id: Uuid,
    ) -> Result<Option<StrategyTaskStepEvidence>, StrategyTaskStepEvidenceRepositoryError> {
        Ok(self.with_records(|records| {
            records
                .iter()
                .find(|evidence| evidence.id == evidence_id && evidence.source == "query_data")
                .cloned()
        }))
    }
}

pub type SharedStrategyTaskStepEvidenceRepository =
    Arc<dyn StrategyTaskStepEvidenceRepository + Send + Sync>;

#[derive(Debug, Error)]
pub enum StrategyTaskStepEvidenceUseCaseError {
    #[error(transparent)]
    Repository(#[from] StrategyTaskStepEvidenceRepositoryError),
}

#[derive(Clone)]
pub struct StrategyTaskStepEvidenceUseCases {
    repository: SharedStrategyTaskStepEvidenceRepository,
}

impl StrategyTaskStepEvidenceUseCases {
    pub fn new(repository: SharedStrategyTaskStepEvidenceRepository) -> Self {
        Self { repository }
    }

    pub async fn record_query_data(
        &self,
        execution_step_id: Uuid,
        instrument_id: &str,
        from: NaiveDate,
        to: NaiveDate,
        timeframe: &str,
        bars: Vec<QueryDataBar>,
    ) -> Result<(), StrategyTaskStepEvidenceUseCaseError> {
        let total_bars = bars.len();
        let truncated = total_bars > MAX_SNAPSHOT_BARS;
        let snapshot_bars = if truncated {
            &bars[total_bars - MAX_SNAPSHOT_BARS..]
        } else {
            &bars
        };
        // 価格データに独立した公表時刻はないため、最新バーの時刻を両者に使う。
        let latest_bar_at = bars.last().map(|bar| bar.timestamp);
        let snapshot = json!({
            "instrument_id": instrument_id,
            "from": from,
            "to": to,
            "timeframe": timeframe,
            "bars": snapshot_bars,
            "total_bars": total_bars,
            "truncated": truncated,
        });

        self.repository
            .insert(StrategyTaskStepEvidence {
                id: Uuid::new_v4(),
                execution_step_id,
                source: "query_data".to_string(),
                source_ref: instrument_id.to_string(),
                observed_at: Utc::now().fixed_offset(),
                published_at: latest_bar_at,
                effective_at: latest_bar_at,
                snapshot,
            })
            .await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use chrono::{DateTime, NaiveDate};
    use rstest::rstest;
    use serde_json::json;
    use uuid::Uuid;

    use super::{
        FakeStrategyTaskStepEvidenceRepository, MAX_SNAPSHOT_BARS, QueryDataBar,
        SharedStrategyTaskStepEvidenceRepository, StrategyTaskStepEvidence,
        StrategyTaskStepEvidenceUseCases,
    };

    fn bar_at(offset: i64) -> QueryDataBar {
        let base = DateTime::parse_from_rfc3339("2030-01-01T00:00:00Z").expect("base timestamp");
        QueryDataBar {
            timestamp: base + chrono::Duration::seconds(offset),
            open: offset as f64,
            high: offset as f64 + 1.0,
            low: offset as f64 - 1.0,
            close: offset as f64 + 0.5,
            volume: offset,
        }
    }

    fn normalize(mut evidence: StrategyTaskStepEvidence) -> StrategyTaskStepEvidence {
        evidence.id = Uuid::nil();
        evidence.observed_at =
            DateTime::parse_from_rfc3339("2000-01-01T00:00:00Z").expect("timestamp sentinel");
        evidence
    }

    #[rstest]
    #[case::empty("1d", 0, 0, false)]
    #[case::truncated_keeps_the_latest_bars("1m", MAX_SNAPSHOT_BARS + 1, 1, true)]
    #[tokio::test]
    async fn record_query_data_stores_the_expected_snapshot(
        #[case] timeframe: &str,
        #[case] total_bars: usize,
        #[case] first_expected_bar: usize,
        #[case] truncated: bool,
    ) {
        let repository = FakeStrategyTaskStepEvidenceRepository::default();
        let repository_for_use_case: SharedStrategyTaskStepEvidenceRepository =
            Arc::new(repository.clone());
        let use_cases = StrategyTaskStepEvidenceUseCases::new(repository_for_use_case);
        let execution_step_id = Uuid::new_v4();
        let from = NaiveDate::from_ymd_opt(2030, 1, 1).expect("from date");
        let to = NaiveDate::from_ymd_opt(2030, 1, 2).expect("to date");
        let bars: Vec<_> = (0..total_bars)
            .map(|offset| bar_at(offset as i64))
            .collect();
        let expected_bars = bars[first_expected_bar..].to_vec();
        let latest_bar_at = bars.last().map(|bar| bar.timestamp);

        use_cases
            .record_query_data(
                execution_step_id,
                "fictional-instrument",
                from,
                to,
                timeframe,
                bars,
            )
            .await
            .expect("record evidence");

        let actual = repository
            .records
            .lock()
            .expect("record lock")
            .first()
            .cloned()
            .expect("recorded evidence");
        assert_eq!(
            normalize(actual),
            StrategyTaskStepEvidence {
                id: Uuid::nil(),
                execution_step_id,
                source: "query_data".to_string(),
                source_ref: "fictional-instrument".to_string(),
                observed_at: DateTime::parse_from_rfc3339("2000-01-01T00:00:00Z")
                    .expect("timestamp sentinel"),
                published_at: latest_bar_at,
                effective_at: latest_bar_at,
                snapshot: json!({
                    "instrument_id": "fictional-instrument",
                    "from": from,
                    "to": to,
                    "timeframe": timeframe,
                    "bars": expected_bars,
                    "total_bars": total_bars,
                    "truncated": truncated,
                }),
            },
        );
    }
}
