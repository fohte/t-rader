//! 信用残の定期取り込みを application のユースケースへ委譲する poll task。

use std::time::Duration;

use chrono::Utc;
use core_application::margin::MarginUseCases;
use tokio::task::JoinHandle;

use crate::data_provider::SharedMarginSource;

/// poll task のデフォルト実行間隔 (1 日)。日次更新の J-Quants データに対して十分な頻度。
pub const DEFAULT_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);

/// poll task を起動する。1 回目は即実行し、その後 `interval` で繰り返す。
pub fn spawn_poll(
    use_cases: MarginUseCases,
    source: SharedMarginSource,
    interval: Duration,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(interval);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            ticker.tick().await;
            let today = Utc::now().date_naive();
            match use_cases.ingest(source.as_ref(), today).await {
                Ok((interest_stats, alert_stats)) => {
                    tracing::info!(
                        ?interest_stats,
                        ?alert_stats,
                        "margin ingest cycle completed"
                    );
                }
                Err(error) => {
                    tracing::warn!(%error, "margin ingest cycle failed");
                }
            }
        }
    })
}
