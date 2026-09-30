//! 決算発表予定日を日付指定で定期的に取り込む定期タスク。

use std::time::Duration;

use chrono::Utc;
use core_application::earnings_schedule::{
    EarningsScheduleIngestStats, EarningsScheduleUseCaseError,
};
use core_application::earnings_schedule_source::{
    EarningsScheduleSource, SharedEarningsScheduleSource,
};
use gateway_postgres::DatabaseHandle;
use sea_orm::DatabaseConnection;
use tokio::task::JoinHandle;

/// poll task のデフォルト実行間隔。決算発表予定日の更新頻度 (日次) に合わせて 1 日とする。
pub const DEFAULT_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);

/// 決算発表予定日を取り込む 1 tick。
pub async fn run_ingest_cycle(
    db: &DatabaseConnection,
    source: &dyn EarningsScheduleSource,
) -> Result<EarningsScheduleIngestStats, EarningsScheduleUseCaseError> {
    crate::services::use_cases::build_use_cases(DatabaseHandle::from(db.clone()))
        .earnings_schedules()
        .run_ingest_cycle(source, Utc::now().date_naive())
        .await
}

/// poll task を起動する。1 回目は即実行し、その後 `interval` で繰り返す。
pub fn spawn_poll(
    db: DatabaseConnection,
    source: SharedEarningsScheduleSource,
    interval: Duration,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(interval);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            ticker.tick().await;
            match run_ingest_cycle(&db, source.as_ref()).await {
                Ok(stats) => {
                    tracing::debug!(
                        days_attempted = stats.days_attempted,
                        upserted = stats.upserted,
                        "earnings date ingest cycle completed",
                    );
                }
                Err(err) => {
                    tracing::warn!(%err, "earnings date ingest cycle failed");
                }
            }
        }
    })
}
