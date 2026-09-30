//! `/fins/summary` (財務情報) を日付指定で定期的に取り込む定期タスク。

use std::time::Duration;

use chrono::{NaiveDate, Utc};
use core_application::financial_summary::{
    FinancialSummaryIngestStats, FinancialSummaryUseCaseError,
};
use core_application::financial_summary_source::{
    FinancialSummarySource, SharedFinancialSummarySource,
};
use gateway_postgres::DatabaseHandle;
use sea_orm::DatabaseConnection;
use tokio::task::JoinHandle;

/// poll task のデフォルト実行間隔。財務情報の更新頻度 (日次) に合わせて 1 日とする。
pub const DEFAULT_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);

/// 財務情報を取り込む 1 tick。
pub async fn run_ingest_cycle(
    db: &DatabaseConnection,
    source: &dyn FinancialSummarySource,
    today: NaiveDate,
) -> Result<FinancialSummaryIngestStats, FinancialSummaryUseCaseError> {
    crate::services::use_cases::build_use_cases(DatabaseHandle::from(db.clone()))
        .financial_summaries()
        .run_ingest_cycle(source, today)
        .await
}

/// poll task を起動する。1 回目は即実行し、その後 `interval` で繰り返す。
pub fn spawn_poll(
    db: DatabaseConnection,
    source: SharedFinancialSummarySource,
    interval: Duration,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(interval);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            ticker.tick().await;
            match run_ingest_cycle(&db, source.as_ref(), Utc::now().date_naive()).await {
                Ok(stats) => {
                    tracing::debug!(
                        days_attempted = stats.days_attempted,
                        upserted = stats.upserted,
                        "fin summary ingest cycle completed",
                    );
                }
                Err(err) => {
                    tracing::warn!(%err, "fin summary ingest cycle failed");
                }
            }
        }
    })
}
