//! 決算発表予定日を日付指定で取り込む処理。

use chrono::Utc;
use core_application::earnings_schedule::{
    EarningsScheduleIngestStats, EarningsScheduleUseCaseError,
};
use core_application::earnings_schedule_source::EarningsScheduleSource;
use gateway_postgres::DatabaseHandle;
use sea_orm::DatabaseConnection;

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
