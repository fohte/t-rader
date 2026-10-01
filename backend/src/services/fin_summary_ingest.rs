//! `/fins/summary` (財務情報) を日付指定で取り込む処理。

use chrono::NaiveDate;
use core_application::financial_summary::{
    FinancialSummaryIngestStats, FinancialSummaryUseCaseError,
};
use core_application::financial_summary_source::FinancialSummarySource;
use gateway_postgres::DatabaseHandle;
use sea_orm::DatabaseConnection;

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
