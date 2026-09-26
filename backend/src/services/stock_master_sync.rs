//! J-Quants の全上場銘柄マスタを日次で `stock` に同期する定期タスク。
//!
//! 全上場銘柄を `stock` に upsert し、名前・市場区分・業種・商品区分を最新に保つ。
//! master に含まれなくなった行 (上場廃止した保有銘柄等) は削除せずそのまま残す。

use std::collections::HashSet;
use std::time::Duration;

use chrono::Utc;
use core_domain::equity_master::EquityMasterEntry;
use sea_orm::ActiveValue::Set;
use sea_orm::sea_query::OnConflict;
use sea_orm::{DatabaseConnection, EntityTrait};
use tokio::task::JoinHandle;

use crate::data_provider::{EquityMasterSource, SharedEquityMasterSource};
use crate::entities::{sector, stock};
use crate::error::AppError;

/// poll task のデフォルト実行間隔。全銘柄マスタの更新頻度 (日次) に合わせて 1 日とする。
pub const DEFAULT_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);

/// poll サイクルの結果統計
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SyncStats {
    pub stocks_upserted: usize,
}

async fn upsert_sectors(
    db: &impl sea_orm::ConnectionTrait,
    entries: &[EquityMasterEntry],
) -> Result<(), sea_orm::DbErr> {
    let names: HashSet<&str> = entries
        .iter()
        .filter_map(|e| e.sector_name.as_deref())
        .collect();
    if names.is_empty() {
        return Ok(());
    }

    let models = names.into_iter().map(|name| sector::ActiveModel {
        id: Set(name.to_string()),
        name: Set(name.to_string()),
    });
    // support_returning() が true (Postgres) だと通常の exec() は RETURNING 行を
    // 前提にしており、ON CONFLICT DO NOTHING で行が返らないと DbErr::RecordNotInserted
    // になってしまう。conflict を正常系として扱うため returning 不要な exec を使う
    sector::Entity::insert_many(models)
        .on_conflict(
            OnConflict::column(sector::Column::Id)
                .do_nothing()
                .to_owned(),
        )
        .exec_without_returning(db)
        .await?;
    Ok(())
}

async fn upsert_stocks(
    db: &impl sea_orm::ConnectionTrait,
    entries: &[EquityMasterEntry],
) -> Result<usize, sea_orm::DbErr> {
    if entries.is_empty() {
        return Ok(0);
    }

    let now = Utc::now().fixed_offset();
    let models = entries.iter().map(|e| stock::ActiveModel {
        id: Set(e.id.clone()),
        name: Set(e.name.clone()),
        market: Set(e.market.clone()),
        sector_id: Set(e.sector_name.clone()),
        product_category: Set(e.product_category.clone()),
        created_at: Set(now),
        updated_at: Set(now),
    });

    stock::Entity::insert_many(models)
        .on_conflict(
            OnConflict::column(stock::Column::Id)
                .update_columns([
                    stock::Column::Name,
                    stock::Column::Market,
                    stock::Column::SectorId,
                    stock::Column::ProductCategory,
                    stock::Column::UpdatedAt,
                ])
                .to_owned(),
        )
        .exec_without_returning(db)
        .await?;

    Ok(entries.len())
}

/// 全上場銘柄マスタを取得し、`stock` (および参照先の `sector`) に反映する 1 サイクル。
pub async fn run_sync_cycle(
    db: &impl sea_orm::ConnectionTrait,
    source: &dyn EquityMasterSource,
) -> Result<SyncStats, AppError> {
    let entries = source.fetch_all_equities_master().await?;

    upsert_sectors(db, &entries).await?;
    let stocks_upserted = upsert_stocks(db, &entries).await?;

    Ok(SyncStats { stocks_upserted })
}

/// poll task を起動する。1 回目は即実行し、その後 `interval` で繰り返す。
pub fn spawn_poll(
    db: DatabaseConnection,
    source: SharedEquityMasterSource,
    interval: Duration,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(interval);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            ticker.tick().await;
            match run_sync_cycle(&db, source.as_ref()).await {
                Ok(stats) => {
                    tracing::info!(
                        stocks_upserted = stats.stocks_upserted,
                        "stock master sync cycle completed",
                    );
                }
                Err(err) => {
                    tracing::warn!(%err, "stock master sync cycle failed");
                }
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use sea_orm::ActiveModelTrait;
    use sea_orm::ActiveValue::NotSet;
    use sqlx::PgPool;

    use super::*;
    use crate::data_provider::jquants::mock::{JQuantsMockServer, MockEquitiesMasterEntry};
    use crate::testing::create_test_db;

    async fn fetch_stock(db: &impl sea_orm::ConnectionTrait, id: &str) -> Option<stock::Model> {
        stock::Entity::find_by_id(id.to_string())
            .one(db)
            .await
            .expect("query ok")
    }

    #[sqlx::test(migrations = false)]
    async fn creates_new_stocks_with_sector_and_market(pool: PgPool) {
        let db = create_test_db(pool).await;
        let mock = JQuantsMockServer::start().await;
        let client = mock.client().expect("client");

        mock.equities_master()
            .entries(vec![MockEquitiesMasterEntry {
                code: "72030",
                company_name: "トヨタ自動車",
                market_name: Some("プライム"),
                sector_name: Some("輸送用機器"),
                product_category: Some("011"),
            }])
            .ok()
            .await;

        let stats = run_sync_cycle(&db, &client).await.expect("cycle ok");
        let stock = fetch_stock(&db, "7203").await.expect("stock exists");
        let sector_row = sector::Entity::find_by_id("輸送用機器".to_string())
            .one(&db)
            .await
            .expect("query ok");

        assert_eq!(
            (
                stats,
                stock.name,
                stock.market,
                stock.sector_id,
                stock.product_category,
                sector_row.map(|s| s.name),
            ),
            (
                SyncStats { stocks_upserted: 1 },
                "トヨタ自動車".to_string(),
                Some("プライム".to_string()),
                Some("輸送用機器".to_string()),
                Some("011".to_string()),
                Some("輸送用機器".to_string()),
            )
        );
    }

    #[sqlx::test(migrations = false)]
    async fn updates_existing_stock_fields(pool: PgPool) {
        let db = create_test_db(pool).await;
        let mock = JQuantsMockServer::start().await;
        let client = mock.client().expect("client");
        let previous_timestamp = (Utc::now() - chrono::Duration::days(1)).fixed_offset();

        stock::ActiveModel {
            id: Set("7203".to_string()),
            name: Set("旧社名".to_string()),
            market: Set(None),
            sector_id: Set(None),
            product_category: Set(None),
            created_at: Set(previous_timestamp),
            updated_at: Set(previous_timestamp),
        }
        .insert(&db)
        .await
        .expect("seed stock");
        let before = fetch_stock(&db, "7203").await.expect("stock exists");

        mock.equities_master()
            .entries(vec![MockEquitiesMasterEntry {
                code: "72030",
                company_name: "トヨタ自動車",
                market_name: Some("プライム"),
                sector_name: Some("輸送用機器"),
                product_category: Some("011"),
            }])
            .ok()
            .await;

        run_sync_cycle(&db, &client).await.expect("cycle ok");
        let after = fetch_stock(&db, "7203").await.expect("stock exists");

        assert_eq!(
            (
                after.name,
                after.market,
                after.sector_id,
                after.product_category,
                after.created_at,
                after.updated_at > before.updated_at,
            ),
            (
                "トヨタ自動車".to_string(),
                Some("プライム".to_string()),
                Some("輸送用機器".to_string()),
                Some("011".to_string()),
                before.created_at,
                true,
            )
        );
    }

    #[sqlx::test(migrations = false)]
    async fn leaves_stocks_not_present_in_master_untouched(pool: PgPool) {
        let db = create_test_db(pool).await;
        let mock = JQuantsMockServer::start().await;
        let client = mock.client().expect("client");

        stock::ActiveModel {
            id: Set("9999".to_string()),
            name: Set("上場廃止銘柄".to_string()),
            market: Set(None),
            sector_id: Set(None),
            product_category: Set(None),
            created_at: NotSet,
            updated_at: NotSet,
        }
        .insert(&db)
        .await
        .expect("seed stock");

        mock.equities_master().entries(vec![]).ok().await;

        let stats = run_sync_cycle(&db, &client).await.expect("cycle ok");
        let stock = fetch_stock(&db, "9999").await.expect("stock still exists");

        assert_eq!(
            (stats, stock.name),
            (SyncStats { stocks_upserted: 0 }, "上場廃止銘柄".to_string())
        );
    }

    #[sqlx::test(migrations = false)]
    async fn shares_sector_across_multiple_stocks(pool: PgPool) {
        let db = create_test_db(pool).await;
        let mock = JQuantsMockServer::start().await;
        let client = mock.client().expect("client");

        mock.equities_master()
            .entries(vec![
                MockEquitiesMasterEntry {
                    code: "72030",
                    company_name: "トヨタ自動車",
                    market_name: Some("プライム"),
                    sector_name: Some("輸送用機器"),
                    product_category: Some("011"),
                },
                MockEquitiesMasterEntry {
                    code: "72670",
                    company_name: "ホンダ",
                    market_name: Some("プライム"),
                    sector_name: Some("輸送用機器"),
                    product_category: Some("011"),
                },
            ])
            .ok()
            .await;

        let stats = run_sync_cycle(&db, &client).await.expect("cycle ok");
        let toyota = fetch_stock(&db, "7203").await.expect("stock exists");
        let honda = fetch_stock(&db, "7267").await.expect("stock exists");

        assert_eq!(
            (stats, toyota.sector_id, honda.sector_id),
            (
                SyncStats { stocks_upserted: 2 },
                Some("輸送用機器".to_string()),
                Some("輸送用機器".to_string()),
            )
        );
    }

    #[sqlx::test(migrations = false)]
    async fn leaves_sector_id_null_when_master_has_no_sector(pool: PgPool) {
        let db = create_test_db(pool).await;
        let mock = JQuantsMockServer::start().await;
        let client = mock.client().expect("client");

        mock.equities_master()
            .entries(vec![MockEquitiesMasterEntry {
                code: "13000",
                company_name: "テスト ETF",
                market_name: None,
                sector_name: None,
                product_category: Some("014"),
            }])
            .ok()
            .await;

        let stats = run_sync_cycle(&db, &client).await.expect("cycle ok");
        let stock = fetch_stock(&db, "1300").await.expect("stock exists");

        assert_eq!(
            (stats, stock.market, stock.sector_id, stock.product_category),
            (
                SyncStats { stocks_upserted: 1 },
                None,
                None,
                Some("014".to_string()),
            )
        );
    }
}
