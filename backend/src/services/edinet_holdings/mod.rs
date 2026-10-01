//! 保有構造の書類を取り込む処理。

use chrono::NaiveDate;
use core_application::shareholding_structure::ShareholdingStructureUseCaseError;
use core_application::shareholding_structure_source::ShareholdingStructureSource;
use gateway_postgres::DatabaseHandle;
use sea_orm::DatabaseConnection;

pub use core_application::shareholding_structure::ShareholdingStructureIngestStats as IngestStats;

/// 保有構造の取り込み 1 サイクル。
pub async fn run_ingest_cycle(
    db: &DatabaseConnection,
    source: &dyn ShareholdingStructureSource,
    today: NaiveDate,
) -> Result<IngestStats, ShareholdingStructureUseCaseError> {
    crate::services::use_cases::build_use_cases(DatabaseHandle::from(db.clone()))
        .shareholding_structures()
        .run_ingest_cycle(source, today)
        .await
}
