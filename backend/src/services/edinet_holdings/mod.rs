//! 保有構造の書類を定期的に取り込むバックグラウンドタスク。

use std::time::Duration;

use chrono::{NaiveDate, Utc};
use core_application::shareholding_structure::ShareholdingStructureUseCaseError;
use core_application::shareholding_structure_source::ShareholdingStructureSource;
use gateway_postgres::DatabaseHandle;
use sea_orm::DatabaseConnection;
use tokio::task::JoinHandle;

pub use core_application::shareholding_structure::ShareholdingStructureIngestStats as IngestStats;

/// ポーリング実行間隔。日次で更新されるデータに対して 1 日間隔とする。
pub const DEFAULT_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);

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

/// poll task を起動する。初回は即実行し、その後 `interval` で繰り返す。
pub fn spawn_poll(
    db: DatabaseConnection,
    source: core_application::shareholding_structure_source::SharedShareholdingStructureSource,
    interval: Duration,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(interval);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            ticker.tick().await;
            match run_ingest_cycle(&db, source.as_ref(), Utc::now().date_naive()).await {
                Ok(stats) => {
                    tracing::debug!(?stats, "保有構造の取り込みが完了しました")
                }
                Err(error) => {
                    tracing::warn!(%error, "保有構造の取り込みに失敗しました")
                }
            }
        }
    })
}
