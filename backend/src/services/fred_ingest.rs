//! FRED の観測値取り込みを定期実行する。

use std::time::Duration;

use core_application::indicator_observation::{
    IndicatorObservationIngestSeriesResult, IndicatorObservationUseCases,
};
use core_application::indicator_observation_source::SharedIndicatorObservationSource;
use tokio::task::JoinHandle;

/// poll task のデフォルト実行間隔。
pub const DEFAULT_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);

/// poll task を起動する。1 回目は即実行し、その後 `interval` で繰り返す。
pub fn spawn_poll(
    use_cases: IndicatorObservationUseCases,
    source: SharedIndicatorObservationSource,
    interval: Duration,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(interval);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            ticker.tick().await;
            let result = use_cases.ingest(source.as_ref()).await;
            for outcome in result.series {
                match outcome {
                    IndicatorObservationIngestSeriesResult::Succeeded {
                        series_id,
                        upserted,
                    } => {
                        tracing::debug!(
                            series = %series_id,
                            upserted,
                            "FRED ingest cycle completed",
                        );
                    }
                    IndicatorObservationIngestSeriesResult::Failed { series_id, error } => {
                        tracing::warn!(series = %series_id, %error, "FRED ingest cycle failed");
                    }
                }
            }
        }
    })
}
