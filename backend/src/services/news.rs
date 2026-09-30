use std::time::Duration;

use core_application::news::NewsUseCases;
use core_application::news_aggregator::SharedNewsAggregator;
use tokio::task::JoinHandle;

/// poll task を起動する。1 回目は即実行し、その後 `interval` で繰り返す
pub fn spawn_poll(
    use_cases: NewsUseCases,
    aggregator: SharedNewsAggregator,
    interval: Duration,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(interval);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            ticker.tick().await;
            match use_cases.run_aggregation_cycle(aggregator.as_ref()).await {
                Ok(stats) => {
                    tracing::debug!(fetched = stats.fetched, "news aggregation cycle completed",);
                }
                Err(err) => {
                    tracing::warn!(%err, "news aggregation cycle failed");
                }
            }
        }
    })
}
