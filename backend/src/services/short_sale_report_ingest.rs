//! J-Quants `/markets/short-sale-report` を日次で取り込むバックグラウンドタスク。

use std::time::Duration;

use chrono::Utc;
use core_application::short_sale_report::ShortSaleReportUseCases;
use core_application::short_selling_source::SharedShortSellingSource;
use tokio::task::JoinHandle;

/// poll task のデフォルト実行間隔。空売り残高報告は日次更新のデータのため、
/// リアルタイム性を重視しないプロダクト方針も踏まえ 1 日間隔にする。
pub const DEFAULT_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);

/// poll task を起動する。1 回目は即実行し、その後 `interval` で繰り返す。
pub fn spawn_poll(
    use_cases: ShortSaleReportUseCases,
    source: SharedShortSellingSource,
    interval: Duration,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(interval);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            ticker.tick().await;
            match use_cases
                .ingest(source.as_ref(), Utc::now().date_naive())
                .await
            {
                Ok(stats) => tracing::debug!(
                    days_fetched = stats.days_fetched,
                    rows_upserted = stats.rows_upserted,
                    "short sale report ingest cycle completed",
                ),
                Err(error) => tracing::warn!(%error, "short sale report ingest cycle failed"),
            }
        }
    })
}
