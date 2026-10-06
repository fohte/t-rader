//! 銘柄マスタ同期 use case の DB integration test。

#[cfg(test)]
mod tests {
    use crate::services::use_cases::build_use_cases;
    use chrono::Utc;
    use core_application::equity_master::EquityMasterSyncStats as SyncStats;
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
            key: Set("sample-derived-axis".into()),
            name: Set("Sample synchronized axis".into()),
            description: Set("Synthetic test axis".into()),
            derive_from: Set(Some("tse_sector33".into())),
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

        let stats = build_use_cases(db.clone())
            .equity_master()
            .sync(&client)
            .await
            .expect("cycle ok");
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
                group.code,
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

        build_use_cases(db.clone())
            .equity_master()
            .sync(&client)
            .await
            .expect("cycle ok");
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

        let stats = build_use_cases(db.clone())
            .equity_master()
            .sync(&client)
            .await
            .expect("cycle ok");
        let stock = fetch_stock(&db, "9999").await.expect("stock still exists");

        assert_eq!(
            (stats, stock.name),
            (SyncStats { stocks_upserted: 0 }, "架空銘柄".to_string())
        );
    }
}
