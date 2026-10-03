//! J-Quants の全上場銘柄マスタを `stock` に同期する処理。
//!
//! 全上場銘柄を `stock` に upsert し、名前・市場区分・商品区分を最新に保つ。
//! 業種は `sync_source = 'jquants'` の分類軸にグループと所属として同期する。
//! master に含まれなくなった行 (上場廃止した保有銘柄等) は削除せずそのまま残す。

use core_application::equity_master::EquityMasterUseCaseError;
use core_application::equity_master_source::EquityMasterSource;
use gateway_postgres::DatabaseHandle;

pub use core_application::equity_master::EquityMasterSyncStats as SyncStats;

/// 全上場銘柄マスタを取得し、`stock` と J-Quants 分類軸のグループ所属に反映する。
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
    use gateway_postgres::entities::{group_axis, stock, stock_group};
    use sea_orm::ActiveModelTrait;
    use sea_orm::ActiveValue::NotSet;
    use sea_orm::ActiveValue::Set;
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
    async fn fetch_stock(db: &impl sea_orm::ConnectionTrait, id: &str) -> Option<stock::Model> {
        stock::Entity::find_by_id(id.to_string())
            .one(db)
            .await
            .expect("query ok")
    }

    #[backend_test_macros::database_test]
    async fn creates_new_stocks_and_synchronizes_group_code(db: gateway_postgres::DatabaseHandle) {
        let mock = JQuantsMockServer::start().await;
        let client = mock.client().expect("client");
        let axis = group_axis::Entity::insert(group_axis::ActiveModel {
            id: Set(uuid::Uuid::new_v4()),
            key: Set("sample-jquants-axis".into()),
            name: Set("Sample synchronized axis".into()),
            description: Set("Synthetic test axis".into()),
            sync_source: Set(Some("jquants".into())),
        })
        .exec_with_returning(&db)
        .await
        .expect("insert group axis");

        mock.equities_master()
            .entries(vec![MockEquitiesMasterEntry {
                code: "ZZ990",
                company_name: "架空銘柄",
                market_name: Some("架空市場"),
                sector_name: Some("架空業種"),
                sector_code: Some("1234"),
                product_category: Some("000"),
            }])
            .ok()
            .await;

        let stats = run_sync_cycle(db.clone(), &client).await.expect("cycle ok");
        let stock = fetch_stock(&db, "ZZ99").await.expect("stock exists");
        let group = stock_group::Entity::find()
            .filter(stock_group::Column::AxisId.eq(axis.id))
            .filter(stock_group::Column::Key.eq("架空業種"))
            .one(&db)
            .await
            .expect("query group")
            .expect("group exists");

        assert_eq!(
            (
                stats,
                stock.name,
                stock.market,
                stock.product_category,
                group.sync_source_code,
            ),
            (
                SyncStats { stocks_upserted: 1 },
                "架空銘柄".to_string(),
                Some("架空市場".to_string()),
                Some("000".to_string()),
                Some("1234".to_string()),
            )
        );
    }

    #[backend_test_macros::database_test]
    async fn updates_existing_stock_fields(db: gateway_postgres::DatabaseHandle) {
        let mock = JQuantsMockServer::start().await;
        let client = mock.client().expect("client");
        let previous_timestamp = (Utc::now() - chrono::Duration::days(1)).fixed_offset();

        stock::ActiveModel {
            id: Set("ZZ99".to_string()),
            name: Set("旧社名".to_string()),
            market: Set(None),
            product_category: Set(None),
            created_at: Set(previous_timestamp),
            updated_at: Set(previous_timestamp),
        }
        .insert(&db)
        .await
        .expect("seed stock");
        let before = fetch_stock(&db, "ZZ99").await.expect("stock exists");

        mock.equities_master()
            .entries(vec![MockEquitiesMasterEntry {
                code: "ZZ990",
                company_name: "架空銘柄",
                market_name: Some("架空市場"),
                sector_name: Some("架空業種"),
                sector_code: Some("1234"),
                product_category: Some("000"),
            }])
            .ok()
            .await;

        run_sync_cycle(db.clone(), &client).await.expect("cycle ok");
        let after = fetch_stock(&db, "ZZ99").await.expect("stock exists");

        assert_eq!(
            (
                after.name,
                after.market,
                after.product_category,
                after.created_at,
                after.updated_at > before.updated_at,
            ),
            (
                "架空銘柄".to_string(),
                Some("架空市場".to_string()),
                Some("000".to_string()),
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
            name: Set("架空銘柄".to_string()),
            market: Set(None),
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
            (SyncStats { stocks_upserted: 0 }, "架空銘柄".to_string())
        );
    }
}
