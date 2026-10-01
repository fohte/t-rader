use std::time::Duration;

use core_application::bars::BarsUseCases;
use core_application::market_daily_bar_source::SharedMarketDailyBarSource;
use tokio::task::JoinHandle;

/// poll task のデフォルト実行間隔。
pub const DEFAULT_INTERVAL: Duration = Duration::from_secs(60 * 60);

/// poll task を起動する。1 回目は即実行し、その後 `interval` で繰り返す。
pub fn spawn_poll(
    use_cases: BarsUseCases,
    source: SharedMarketDailyBarSource,
    interval: Duration,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(interval);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            ticker.tick().await;
            match use_cases.run_ingest_cycle(source.as_ref()).await {
                Ok(stats) => {
                    tracing::debug!(
                        days_attempted = stats.days_attempted,
                        bars_upserted = stats.bars_upserted,
                        "daily bars ingest cycle completed",
                    );
                }
                Err(error) => {
                    tracing::warn!(%error, "daily bars ingest cycle failed");
                }
            }
        }
    })
}
