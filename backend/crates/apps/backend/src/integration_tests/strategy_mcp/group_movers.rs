use chrono::{NaiveDate, TimeZone, Utc};
use core_domain::bar::{Bar, Timeframe};
use gateway_postgres::entities::{group_axis, instruments, stock_group, stock_group_member};
use gateway_postgres::repositories::bars::upsert_bars;
use rust_decimal::Decimal;
use sea_orm::ActiveModelTrait;
use sea_orm::ActiveValue::Set;
use serde_json::{Value, json};
use uuid::Uuid;

use super::test_server::StrategyServer;
use super::tests_common::{build_server, insert_strategy};

fn date(year: i32, month: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(year, month, day).expect("valid fixture date")
}

fn bar(instrument_id: &str, date: NaiveDate, close: i64, volume: i64) -> Bar {
    let timestamp = Utc.from_utc_datetime(&date.and_hms_opt(0, 0, 0).expect("valid fixture time"));
    Bar {
        instrument_id: instrument_id.to_string(),
        timeframe: Timeframe::Daily,
        timestamp,
        open: Decimal::from(close),
        high: Decimal::from(close + 1),
        low: Decimal::from(close - 1),
        close: Decimal::from(close),
        volume,
        adjustment_factor: Decimal::ONE,
    }
}

async fn insert_axis(db: &impl sea_orm::ConnectionTrait, key: &str) -> Uuid {
    let id = Uuid::new_v4();
    group_axis::ActiveModel {
        id: Set(id),
        key: Set(key.to_string()),
        name: Set("Sample axis".to_string()),
        description: Set("Sample axis for integration tests".to_string()),
        derive_from: Set(None),
    }
    .insert(db)
    .await
    .expect("insert sample group axis");
    id
}

async fn insert_group(
    db: &impl sea_orm::ConnectionTrait,
    axis_id: Uuid,
    key: &str,
    name: &str,
) -> Uuid {
    let id = Uuid::new_v4();
    stock_group::ActiveModel {
        id: Set(id),
        axis_id: Set(axis_id),
        key: Set(key.to_string()),
        name: Set(name.to_string()),
        description: Set(None),
        code: Set(None),
    }
    .insert(db)
    .await
    .expect("insert sample stock group");
    id
}

async fn insert_stock(db: &impl sea_orm::ConnectionTrait, id: &str, name: &str) {
    instruments::ActiveModel {
        id: Set(id.to_string()),
        name: Set(name.to_string()),
        market: Set("TSE".to_string()),
        sector: Set(None),
    }
    .insert(db)
    .await
    .expect("insert sample instrument");
    crate::testing::insert_test_stock(db, id, name).await;
}

async fn add_member(db: &impl sea_orm::ConnectionTrait, stock_id: &str, group_id: Uuid) {
    stock_group_member::ActiveModel {
        stock_id: Set(stock_id.to_string()),
        group_id: Set(group_id),
        created_at: sea_orm::ActiveValue::NotSet,
    }
    .insert(db)
    .await
    .expect("insert sample group member");
}

#[backend_test_macros::database_test]
async fn list_group_movers_returns_group_metrics_and_top_constituents(
    db: gateway_postgres::DatabaseHandle,
) {
    let axis_id = insert_axis(&db, "sample-axis").await;
    let first_group_id =
        insert_group(&db, axis_id, "sample-group-alpha", "Sample group alpha").await;
    let second_group_id =
        insert_group(&db, axis_id, "sample-group-beta", "Sample group beta").await;
    let single_axis_id = insert_axis(&db, "sample-single-axis").await;
    let single_group_id = insert_group(
        &db,
        single_axis_id,
        "sample-single-group",
        "Sample single group",
    )
    .await;

    for (stock_id, name) in [
        ("demo-stock-a", "Sample stock A"),
        ("demo-stock-b", "Sample stock B"),
        ("demo-stock-c", "Sample stock C"),
        ("demo-stock-d", "Sample stock D"),
        ("demo-stock-e", "Sample stock E"),
        ("demo-stock-f", "Sample stock F"),
        ("demo-stock-g", "Sample stock G"),
    ] {
        insert_stock(&db, stock_id, name).await;
    }

    for stock_id in [
        "demo-stock-a",
        "demo-stock-b",
        "demo-stock-c",
        "demo-stock-e",
        "demo-stock-f",
        "demo-stock-g",
    ] {
        add_member(&db, stock_id, first_group_id).await;
    }
    add_member(&db, "demo-stock-d", second_group_id).await;
    add_member(&db, "demo-stock-d", single_group_id).await;

    upsert_bars(
        &db,
        vec![
            bar("demo-stock-a", date(2025, 1, 31), 100, 1),
            bar("demo-stock-a", date(2025, 2, 3), 80, 1),
            bar("demo-stock-a", date(2025, 2, 4), 120, 1),
            bar("demo-stock-b", date(2025, 1, 31), 100, 1),
            bar("demo-stock-b", date(2025, 2, 3), 110, 1),
            bar("demo-stock-b", date(2025, 2, 4), 90, 1),
            bar("demo-stock-c", date(2025, 1, 31), 100, 2),
            bar("demo-stock-c", date(2025, 2, 3), 110, 2),
            bar("demo-stock-c", date(2025, 2, 4), 90, 2),
            bar("demo-stock-d", date(2025, 1, 31), 100, 2),
            bar("demo-stock-d", date(2025, 2, 3), 100, 2),
            bar("demo-stock-d", date(2025, 2, 4), 150, 2),
            bar("demo-stock-e", date(2025, 1, 31), 100, 1),
            bar("demo-stock-e", date(2025, 2, 3), 110, 1),
            bar("demo-stock-e", date(2025, 2, 4), 90, 1),
            bar("demo-stock-f", date(2025, 1, 31), 100, 1),
            bar("demo-stock-f", date(2025, 2, 3), 110, 1),
            bar("demo-stock-f", date(2025, 2, 4), 90, 1),
            bar("demo-stock-g", date(2025, 1, 31), 100, 2),
            bar("demo-stock-g", date(2025, 2, 3), 100, 2),
            bar("demo-stock-g", date(2025, 2, 4), 100, 2),
        ],
    )
    .await
    .expect("insert sample daily bars");

    let strategy_id = insert_strategy(&db, "Sample strategy").await;
    let server: StrategyServer = build_server(db);
    let all_groups = server
        .invoke::<_, Value>(
            "list_group_movers",
            strategy_id,
            json!({
                "axis_key": "sample-axis",
                "from": "2025-02-03",
                "to": "2025-02-04",
            }),
            None,
            None,
        )
        .await
        .expect("list group movers")
        .as_json()
        .clone();
    let filtered_groups = server
        .invoke::<_, Value>(
            "list_group_movers",
            strategy_id,
            json!({
                "axis_key": "sample-axis",
                "from": "2025-02-03",
                "to": "2025-02-04",
                "min_avg_turnover": 150,
            }),
            None,
            None,
        )
        .await
        .expect("list group movers with turnover minimum")
        .as_json()
        .clone();
    let single_day = server
        .invoke::<_, Value>(
            "list_group_movers",
            strategy_id,
            json!({
                "axis_key": "sample-single-axis",
                "from": "2025-02-04",
                "to": "2025-02-04",
            }),
            None,
            None,
        )
        .await
        .expect("list single-day group movers")
        .as_json()
        .clone();
    let reversed_range = server
        .invoke::<_, Value>(
            "list_group_movers",
            strategy_id,
            json!({
                "axis_key": "sample-axis",
                "from": "2025-02-04",
                "to": "2025-02-03",
            }),
            None,
            None,
        )
        .await
        .map(|_| ());
    let negative_turnover = server
        .invoke::<_, Value>(
            "list_group_movers",
            strategy_id,
            json!({
                "axis_key": "sample-axis",
                "from": "2025-02-03",
                "to": "2025-02-04",
                "min_avg_turnover": -1,
            }),
            None,
            None,
        )
        .await
        .map(|_| ());

    assert_eq!(
        (
            all_groups,
            filtered_groups,
            single_day,
            reversed_range,
            negative_turnover
        ),
        (
            json!({
                "axis_key": "sample-axis",
                "groups": [
                    {
                        "group_key": "sample-group-beta",
                        "name": "Sample group beta",
                        "constituent_count": 1,
                        "median_change_rate": 0.5,
                        "turnover_weighted_change_rate": 0.5,
                        "total_avg_turnover": 250.0,
                        "top_stocks": [{
                            "stock_id": "demo-stock-d",
                            "name": "Sample stock D",
                            "change_rate": 0.5,
                            "avg_turnover": 250.0,
                        }],
                    },
                    {
                        "group_key": "sample-group-alpha",
                        "name": "Sample group alpha",
                        "constituent_count": 6,
                        "median_change_rate": -0.1,
                        "turnover_weighted_change_rate": -0.0375,
                        "total_avg_turnover": 800.0,
                        "top_stocks": [
                            {
                                "stock_id": "demo-stock-c",
                                "name": "Sample stock C",
                                "change_rate": -0.1,
                                "avg_turnover": 200.0,
                            },
                            {
                                "stock_id": "demo-stock-g",
                                "name": "Sample stock G",
                                "change_rate": 0.0,
                                "avg_turnover": 200.0,
                            },
                            {
                                "stock_id": "demo-stock-a",
                                "name": "Sample stock A",
                                "change_rate": 0.2,
                                "avg_turnover": 100.0,
                            },
                            {
                                "stock_id": "demo-stock-b",
                                "name": "Sample stock B",
                                "change_rate": -0.1,
                                "avg_turnover": 100.0,
                            },
                            {
                                "stock_id": "demo-stock-e",
                                "name": "Sample stock E",
                                "change_rate": -0.1,
                                "avg_turnover": 100.0,
                            },
                        ],
                    },
                ],
            }),
            json!({
                "axis_key": "sample-axis",
                "groups": [
                    {
                        "group_key": "sample-group-beta",
                        "name": "Sample group beta",
                        "constituent_count": 1,
                        "median_change_rate": 0.5,
                        "turnover_weighted_change_rate": 0.5,
                        "total_avg_turnover": 250.0,
                        "top_stocks": [{
                            "stock_id": "demo-stock-d",
                            "name": "Sample stock D",
                            "change_rate": 0.5,
                            "avg_turnover": 250.0,
                        }],
                    },
                    {
                        "group_key": "sample-group-alpha",
                        "name": "Sample group alpha",
                        "constituent_count": 2,
                        "median_change_rate": -0.05,
                        "turnover_weighted_change_rate": -0.049999999999999996,
                        "total_avg_turnover": 400.0,
                        "top_stocks": [
                            {
                                "stock_id": "demo-stock-c",
                                "name": "Sample stock C",
                                "change_rate": -0.1,
                                "avg_turnover": 200.0,
                            },
                            {
                                "stock_id": "demo-stock-g",
                                "name": "Sample stock G",
                                "change_rate": 0.0,
                                "avg_turnover": 200.0,
                            },
                        ],
                    },
                ],
            }),
            json!({
                "axis_key": "sample-single-axis",
                "groups": [{
                    "group_key": "sample-single-group",
                    "name": "Sample single group",
                    "constituent_count": 1,
                    "median_change_rate": 0.5,
                    "turnover_weighted_change_rate": 0.5,
                    "total_avg_turnover": 300.0,
                    "top_stocks": [{
                        "stock_id": "demo-stock-d",
                        "name": "Sample stock D",
                        "change_rate": 0.5,
                        "avg_turnover": 300.0,
                    }],
                }],
            }),
            Err(rmcp::ErrorData::invalid_params(
                "from must be on or before to",
                None,
            )),
            Err(rmcp::ErrorData::invalid_params(
                "min_avg_turnover must be a non-negative number",
                None,
            )),
        ),
    );
}
