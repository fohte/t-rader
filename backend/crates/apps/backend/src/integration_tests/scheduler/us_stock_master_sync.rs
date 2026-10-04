use async_trait::async_trait;
use core_application::stock_group::CreateStockGroupCommand;
use core_application::stock_registration::RegisterStockCommand;
use core_application::us_stock_master::UsStockMasterSyncStats;
use core_application::us_stock_master_source::{UsStockMasterSource, UsStockMasterSourceError};
use core_domain::stock_id::ForeignStockId;
use core_domain::us_stock_master::UsStockMasterEntry;
use gateway_postgres::entities::{group_axis, instruments, stock};
use sea_orm::ActiveValue::Set;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};

use crate::services::use_cases::build_use_cases;

struct FixedUsStockMasterSource(Vec<UsStockMasterEntry>);

#[async_trait]
impl UsStockMasterSource for FixedUsStockMasterSource {
    async fn fetch_all_us_stocks(
        &self,
    ) -> Result<Vec<UsStockMasterEntry>, UsStockMasterSourceError> {
        Ok(self.0.clone())
    }
}

fn stock_entry(code: &str, name: &str, exchange: &str) -> UsStockMasterEntry {
    UsStockMasterEntry {
        id: ForeignStockId::new("US", code).expect("valid synthetic stock id"),
        name: name.to_owned(),
        exchange: Some(exchange.to_owned()),
    }
}

#[backend_test_macros::database_test]
async fn upserts_registered_stocks_and_preserves_stocks_removed_from_the_list(
    db: gateway_postgres::DatabaseHandle,
) {
    let use_cases = build_use_cases(db.clone());
    for (code, name) in [("QZ7", "登録時の名前"), ("QZ8", "一覧から消えた名前")] {
        use_cases
            .stock_registration()
            .register(RegisterStockCommand {
                country: "US".to_owned(),
                code: code.to_owned(),
                name: name.to_owned(),
                exchange: "Example Exchange".to_owned(),
            })
            .await
            .expect("registration succeeds");
    }

    let source = FixedUsStockMasterSource(vec![
        stock_entry("QZ7", "同期後の名前", "Updated Example Exchange"),
        stock_entry("QZ9", "未登録の架空銘柄", "Example Exchange"),
    ]);
    let sync = use_cases.us_stock_master();
    let first_stats = sync.sync(&source).await.expect("first sync succeeds");
    let second_stats = sync.sync(&source).await.expect("repeat sync succeeds");

    let resolved_ref = use_cases
        .refs()
        .resolve(&[("stock".to_owned(), "US:QZ9".to_owned())])
        .await
        .expect("stock reference resolves")
        .into_iter()
        .map(|item| (item.kind, item.id, item.name))
        .collect::<Vec<_>>();
    group_axis::Entity::insert(group_axis::ActiveModel {
        id: Set(uuid::Uuid::new_v4()),
        key: Set("sample-axis".to_owned()),
        name: Set("架空軸".to_owned()),
        description: Set("架空軸".to_owned()),
        sync_source: Set(None),
    })
    .exec_without_returning(&db)
    .await
    .expect("group axis inserts");
    let stock_groups = use_cases.stock_groups();
    stock_groups
        .create(CreateStockGroupCommand {
            axis_key: "sample-axis".to_owned(),
            key: "sample-group".to_owned(),
            name: "架空グループ".to_owned(),
            description: None,
        })
        .await
        .expect("group creates");
    let group_add_result = stock_groups
        .add_stock("sample-axis", "sample-group", "US:QZ9")
        .await
        .expect("unregistered stock can be added to a group");
    let group_stock_ids = stock_groups
        .list_stock_ids("sample-axis", "sample-group")
        .await
        .expect("group stock ids query succeeds");

    let ids = vec![
        "US:QZ7".to_owned(),
        "US:QZ8".to_owned(),
        "US:QZ9".to_owned(),
    ];
    let stocks = stock::Entity::find()
        .filter(stock::Column::Id.is_in(ids.clone()))
        .order_by_asc(stock::Column::Id)
        .all(&db)
        .await
        .expect("stocks query succeeds")
        .into_iter()
        .map(|item| (item.id, item.name, item.market))
        .collect::<Vec<_>>();
    let instruments = instruments::Entity::find()
        .filter(instruments::Column::Id.is_in(ids))
        .order_by_asc(instruments::Column::Id)
        .all(&db)
        .await
        .expect("instruments query succeeds")
        .into_iter()
        .map(|item| (item.id, item.name, item.market))
        .collect::<Vec<_>>();

    assert_eq!(
        (
            first_stats,
            second_stats,
            stocks,
            instruments,
            resolved_ref,
            group_add_result,
            group_stock_ids,
        ),
        (
            UsStockMasterSyncStats { stocks_upserted: 2 },
            UsStockMasterSyncStats { stocks_upserted: 2 },
            vec![
                (
                    "US:QZ7".to_owned(),
                    "同期後の名前".to_owned(),
                    Some("Updated Example Exchange".to_owned()),
                ),
                (
                    "US:QZ8".to_owned(),
                    "一覧から消えた名前".to_owned(),
                    Some("Example Exchange".to_owned()),
                ),
                (
                    "US:QZ9".to_owned(),
                    "未登録の架空銘柄".to_owned(),
                    Some("Example Exchange".to_owned()),
                ),
            ],
            vec![
                (
                    "US:QZ7".to_owned(),
                    "同期後の名前".to_owned(),
                    "US".to_owned(),
                ),
                (
                    "US:QZ8".to_owned(),
                    "一覧から消えた名前".to_owned(),
                    "US".to_owned(),
                ),
                (
                    "US:QZ9".to_owned(),
                    "未登録の架空銘柄".to_owned(),
                    "US".to_owned(),
                ),
            ],
            vec![(
                "stock".to_owned(),
                "US:QZ9".to_owned(),
                Some("未登録の架空銘柄".to_owned()),
            )],
            true,
            vec!["US:QZ9".to_owned()],
        ),
    );
}
