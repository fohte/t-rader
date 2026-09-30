//! 日次バリュエーション指標を全銘柄分取り込む定期タスク。

use std::time::Duration;

use chrono::Utc;
use core_application::valuation::{IngestStats, ValuationUseCaseError};
use core_application::valuation_source::{SharedValuationSource, ValuationSource};
use gateway_postgres::DatabaseHandle;
use sea_orm::DatabaseConnection;
use tokio::task::JoinHandle;

/// poll task のデフォルト実行間隔。
pub const DEFAULT_INTERVAL: Duration = Duration::from_secs(60 * 60);

/// バリュエーション指標を取り込む 1 tick。
pub async fn run_ingest_cycle(
    db: &DatabaseConnection,
    source: &dyn ValuationSource,
) -> Result<IngestStats, ValuationUseCaseError> {
    crate::services::use_cases::build_use_cases(DatabaseHandle::from(db.clone()))
        .valuations()
        .run_ingest_cycle(source, Utc::now().date_naive())
        .await
}

/// データソースが設定された場合に poll task を起動する。
pub fn spawn_poll(
    db: DatabaseConnection,
    source: SharedValuationSource,
    interval: Duration,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(interval);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            ticker.tick().await;
            match run_ingest_cycle(&db, source.as_ref()).await {
                Ok(stats) => tracing::debug!(
                    days_attempted = stats.days_attempted,
                    rows_upserted = stats.rows_upserted,
                    "valuation ingest cycle completed",
                ),
                Err(error) => tracing::warn!(%error, "valuation ingest cycle failed"),
            }
        }
    })
}
