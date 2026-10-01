//! `query_data` の取得結果を実行ステップの evidence として記録する。

use core_application::strategy_task_step_evidence::{
    QueryDataBar, StrategyTaskStepEvidenceUseCaseError, StrategyTaskStepEvidenceUseCases,
};
use uuid::Uuid;

use super::dto::BarDto;

pub(super) async fn record_query_data(
    use_cases: &StrategyTaskStepEvidenceUseCases,
    execution_step_id: Uuid,
    instrument_id: &str,
    from: chrono::NaiveDate,
    to: chrono::NaiveDate,
    bars: &[BarDto],
) -> Result<(), StrategyTaskStepEvidenceUseCaseError> {
    let bars = bars
        .iter()
        .map(|bar| QueryDataBar {
            timestamp: bar.timestamp,
            open: bar.open,
            high: bar.high,
            low: bar.low,
            close: bar.close,
            volume: bar.volume,
        })
        .collect();

    use_cases
        .record_query_data(execution_step_id, instrument_id, from, to, bars)
        .await
}
