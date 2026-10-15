use chrono::{NaiveDate, TimeZone, Utc};
use gateway_postgres::DatabaseHandle;
use gateway_postgres::entities::{
    bars, group_axis, instruments, stock, stock_group, stock_group_member, valuation,
};
use rust_decimal::Decimal;
use sea_orm::{ActiveModelTrait, ActiveValue::Set, EntityTrait};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::testing::create_test_server;

fn date(day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(2098, 1, day).expect("valid test date")
}

async fn insert_stock(db: &DatabaseHandle, id: &str, name: &str) {
    stock::ActiveModel {
        id: Set(id.to_string()),
        name: Set(name.to_string()),
        market: Set(Some("OTHER".to_string())),
        product_category: Set(None),
        created_at: Default::default(),
        updated_at: Default::default(),
    }
    .insert(db)
    .await
    .expect("insert widget stock");
    instruments::Entity::insert(instruments::ActiveModel {
        id: Set(id.to_string()),
        name: Set(name.to_string()),
        market: Set("OTHER".to_string()),
        sector: Set(None),
    })
    .exec_without_returning(db)
    .await
    .expect("insert widget instrument");
}

async fn insert_bar(db: &DatabaseHandle, id: &str, day: u32, close: i64) {
    let timestamp = date(day)
        .and_hms_opt(0, 0, 0)
        .map(|timestamp| Utc.from_utc_datetime(&timestamp).fixed_offset())
        .expect("valid test timestamp");
    let close = Decimal::from(close);
    bars::Entity::insert(bars::ActiveModel {
        instrument_id: Set(id.to_string()),
        timeframe: Set("1d".to_string()),
        timestamp: Set(timestamp),
        open: Set(close),
        high: Set(close),
        low: Set(close),
        close: Set(close),
        volume: Set(100),
        adjustment_factor: Set(Decimal::ONE),
    })
    .exec_without_returning(db)
    .await
    .expect("insert widget bar");
}

async fn insert_valuation(db: &DatabaseHandle, id: &str, day: u32, mkt_cap: i64) {
    valuation::ActiveModel {
        code: Set(format!("{id}0")),
        date: Set(date(day)),
        eps: Set(None),
        fwd_eps: Set(None),
        bps: Set(None),
        roe: Set(None),
        fwd_roe: Set(None),
        per: Set(None),
        fwd_per: Set(None),
        pbr: Set(None),
        mkt_cap: Set(Some(Decimal::from(mkt_cap))),
    }
    .insert(db)
    .await
    .expect("insert widget valuation");
}

#[backend_test_macros::database_test]
async fn query_widget_returns_group_weighted_values_and_formula_data(db: DatabaseHandle) {
    insert_stock(&db, "D001", "Stock Alpha").await;
    insert_stock(&db, "D002", "Stock Beta").await;
    insert_stock(&db, "D003", "Stock Gamma").await;
    for id in ["D001", "D002", "D003"] {
        insert_bar(&db, id, 1, 100).await;
    }
    insert_bar(&db, "D001", 2, 110).await;
    insert_bar(&db, "D002", 2, 90).await;
    insert_bar(&db, "D003", 2, 120).await;
    insert_valuation(&db, "D001", 2, 1_000).await;
    insert_valuation(&db, "D002", 2, 3_000).await;
    insert_valuation(&db, "D003", 2, 1_000).await;

    let axis_id = Uuid::new_v4();
    group_axis::ActiveModel {
        id: Set(axis_id),
        key: Set("demo-axis".to_string()),
        name: Set("Demo Axis".to_string()),
        description: Set("Demo group axis".to_string()),
        derive_from: Set(None),
    }
    .insert(&db)
    .await
    .expect("insert widget group axis");
    let group_id = Uuid::new_v4();
    stock_group::ActiveModel {
        id: Set(group_id),
        axis_id: Set(axis_id),
        key: Set("group-alpha".to_string()),
        name: Set("Group Alpha".to_string()),
        description: Set(None),
        code: Set(None),
    }
    .insert(&db)
    .await
    .expect("insert widget group");
    for id in ["D001", "D002"] {
        stock_group_member::ActiveModel {
            stock_id: Set(id.to_string()),
            group_id: Set(group_id),
            created_at: Default::default(),
        }
        .insert(&db)
        .await
        .expect("insert widget group member");
    }

    let server = create_test_server(db).await;
    let response = server
        .post("/api/widgets/query")
        .json(&json!({
            "queries": {
                "sector": {"metric": "change", "period": "1d", "by": "group", "axis": "demo-axis", "agg": "mkt_cap_weighted"},
                "market": {"metric": "change", "period": "1d", "by": "none", "agg": "mkt_cap_weighted"},
                "cap": {"metric": "mkt_cap", "by": "group", "axis": "demo-axis", "agg": "sum"}
            },
            "formulas": {"relative": "sector - market"},
            "viz": {"type": "treemap", "size": "cap", "color": "relative"},
            "time": {"as_of": "2098-01-02"}
        }))
        .await;
    let body = response.json::<Value>();

    assert_eq!(
        (response.status_code(), body),
        (
            axum::http::StatusCode::OK,
            json!({
                "as_of": "2098-01-02",
                "rows": [
                    {"key": "group:demo-axis/group-alpha", "name": "Group Alpha", "values": {"cap": 4000, "market": 0, "relative": -0.05, "sector": -0.05}}
                ]
            }),
        ),
    );
}

#[backend_test_macros::database_test]
async fn query_widget_returns_semantic_validation_errors(db: DatabaseHandle) {
    let server = create_test_server(db).await;
    let response = server
        .post("/api/widgets/query")
        .json(&json!({
            "queries": {
                "daily": {"metric": "change", "by": "stock"}
            },
            "viz": {"type": "toplist", "value": "daily", "order": "desc", "limit": 5}
        }))
        .await;
    let body = response.json::<Value>();

    assert_eq!(
        (response.status_code(), body),
        (
            axum::http::StatusCode::BAD_REQUEST,
            json!({
                "error": "queries.daily.period: period is required for change metrics"
            }),
        ),
    );
}
