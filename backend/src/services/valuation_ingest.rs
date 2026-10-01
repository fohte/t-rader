//! 日次バリュエーション指標を全銘柄分取り込む処理。

use chrono::Utc;
use core_application::valuation::{IngestStats, ValuationUseCaseError};
use core_application::valuation_source::ValuationSource;
use gateway_postgres::DatabaseHandle;
use sea_orm::DatabaseConnection;

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
