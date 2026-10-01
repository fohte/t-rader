//! J-Quants の全上場銘柄マスタを `stock` に同期する処理。
//!
//! 全上場銘柄を `stock` に upsert し、名前・市場区分・業種・商品区分を最新に保つ。
//! master に含まれなくなった行 (上場廃止した保有銘柄等) は削除せずそのまま残す。

use core_application::equity_master::EquityMasterUseCaseError;
use core_application::equity_master_source::EquityMasterSource;
use gateway_postgres::DatabaseHandle;

pub use core_application::equity_master::EquityMasterSyncStats as SyncStats;

/// 全上場銘柄マスタを取得し、`stock` (および参照先の `sector`) に反映する 1 サイクル。
pub async fn run_sync_cycle(
    db: impl Into<DatabaseHandle>,
    source: &dyn EquityMasterSource,
) -> Result<SyncStats, EquityMasterUseCaseError> {
    crate::services::use_cases::build_use_cases(db)
        .equity_master()
        .sync(source)
        .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use gateway_jquants::mock::{JQuantsMockServer, MockEquitiesMasterEntry};
    use gateway_postgres::entities::{sector, stock};
    use sea_orm::ActiveModelTrait;
    use sea_orm::ActiveValue::NotSet;
    use sea_orm::ActiveValue::Set;
    use sea_orm::EntityTrait;
    async fn fetch_stock(db: &impl sea_orm::ConnectionTrait, id: &str) -> Option<stock::Model> {
        stock::Entity::find_by_id(id.to_string())
            .one(db)
            .await
            .expect("query ok")
    }

    #[backend_test_macros::database_test]
    async fn creates_new_stocks_with_sector_and_market(db: gateway_postgres::DatabaseHandle) {
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

        let stats = run_sync_cycle(db.clone(), &client).await.expect("cycle ok");
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

    #[backend_test_macros::database_test]
    async fn updates_existing_stock_fields(db: gateway_postgres::DatabaseHandle) {
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

        run_sync_cycle(db.clone(), &client).await.expect("cycle ok");
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

    #[backend_test_macros::database_test]
    async fn leaves_stocks_not_present_in_master_untouched(db: gateway_postgres::DatabaseHandle) {
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

        let stats = run_sync_cycle(db.clone(), &client).await.expect("cycle ok");
        let stock = fetch_stock(&db, "9999").await.expect("stock still exists");

        assert_eq!(
            (stats, stock.name),
            (SyncStats { stocks_upserted: 0 }, "上場廃止銘柄".to_string())
        );
    }

    #[backend_test_macros::database_test]
    async fn shares_sector_across_multiple_stocks(db: gateway_postgres::DatabaseHandle) {
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

        let stats = run_sync_cycle(db.clone(), &client).await.expect("cycle ok");
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

    #[backend_test_macros::database_test]
    async fn leaves_sector_id_null_when_master_has_no_sector(db: gateway_postgres::DatabaseHandle) {
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

        let stats = run_sync_cycle(db.clone(), &client).await.expect("cycle ok");
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
