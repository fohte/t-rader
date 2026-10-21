use chrono::{NaiveDate, TimeZone, Utc};
use core_domain::bar::{Bar, Timeframe};
use gateway_postgres::DatabaseHandle;
use gateway_postgres::entities::{indicator, indicator_observation, instruments, stock, valuation};
use gateway_postgres::repositories::bars::upsert_bars;
use rust_decimal::Decimal;
use sea_orm::{ActiveModelTrait, ActiveValue::Set, EntityTrait};
use serde_json::json;

use super::tests_common::{build_server, insert_strategy};

fn as_of() -> NaiveDate {
    NaiveDate::from_ymd_opt(2099, 1, 5).expect("valid test date")
}

fn date(day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(2099, 1, day).expect("valid test date")
}

async fn insert_indicator(db: &DatabaseHandle, id: &str, name: &str) {
    indicator::ActiveModel {
        id: Set(id.to_string()),
        name: Set(name.to_string()),
        kind: Set("sample-kind".to_string()),
    }
    .insert(db)
    .await
    .expect("insert widget indicator");
}

async fn insert_indicator_observation(db: &DatabaseHandle, id: &str, day: u32, value: i64) {
    indicator_observation::ActiveModel {
        indicator_id: Set(id.to_string()),
        date: Set(date(day)),
        value: Set(Decimal::from(value)),
    }
    .insert(db)
    .await
    .expect("insert widget indicator observation");
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

async fn insert_bars(db: &DatabaseHandle, ids: &[&str]) {
    let timestamp = as_of()
        .and_hms_opt(0, 0, 0)
        .map(|timestamp| Utc.from_utc_datetime(&timestamp))
        .expect("valid test timestamp");
    let close = Decimal::from(100);
    upsert_bars(
        db,
        ids.iter()
            .map(|id| Bar {
                instrument_id: (*id).to_string(),
                timeframe: Timeframe::Daily,
                timestamp,
                open: close,
                high: close,
                low: close,
                close,
                volume: 100,
                adjustment_factor: Decimal::ONE,
            })
            .collect(),
    )
    .await
    .expect("upsert widget bar");
}

async fn insert_market_data(db: &DatabaseHandle) {
    insert_bars(db, &["F001"]).await;
    valuation::ActiveModel {
        code: Set("F0010".to_string()),
        date: Set(as_of()),
        eps: Set(None),
        fwd_eps: Set(None),
        bps: Set(None),
        roe: Set(None),
        fwd_roe: Set(None),
        per: Set(None),
        fwd_per: Set(None),
        pbr: Set(None),
        mkt_cap: Set(Some(Decimal::from(1234))),
    }
    .insert(db)
    .await
    .expect("insert widget valuation");
}

#[backend_test_macros::database_test]
async fn query_widget_returns_resolved_date_and_values_as_rows(db: DatabaseHandle) {
    let strategy_id = insert_strategy(&db, "widget-query").await;
    insert_stock(&db, "F001", "Demo Stock").await;
    insert_market_data(&db).await;
    let server = build_server(db);
    let definition_yaml = indoc::indoc! {"
        queries:
          cap:
            metric: mkt_cap
            by: stock
            universe:
              stocks:
                - F001
        viz:
          type: toplist
          value: cap
          order: desc
          limit: 5
        time:
          as_of: 2099-01-05
    "};

    let result = server
        .query_widget(strategy_id, definition_yaml)
        .await
        .expect("query widget");

    assert_eq!(
        result.as_json().clone(),
        json!({
            "as_of": "2099-01-05",
            "rows": [{
                "key": "stock:F001",
                "name": "Demo Stock",
                "values": {"cap": 1234}
            }]
        }),
    );
}

#[backend_test_macros::database_test]
async fn query_widget_rejects_when_no_daily_bar_date_can_be_resolved(db: DatabaseHandle) {
    let strategy_id = insert_strategy(&db, "widget-no-data").await;
    let server = build_server(db);
    let definition_yaml = indoc::indoc! {"
        queries:
          cap:
            metric: mkt_cap
            by: stock
            universe:
              stocks:
                - F001
        viz:
          type: toplist
          value: cap
          order: desc
          limit: 5
    "};

    let error = server
        .query_widget(strategy_id, definition_yaml)
        .await
        .expect_err("query without any daily bar data");

    assert_eq!(
        error,
        rmcp::ErrorData::invalid_params(
            "no widget data is available on or before the requested date".to_string(),
            None,
        ),
    );
}

#[backend_test_macros::database_test]
async fn query_widget_returns_indicator_table_without_daily_bars(db: DatabaseHandle) {
    let strategy_id = insert_strategy(&db, "widget-indicator").await;
    insert_indicator(&db, "demo-indicator", "Demo Indicator").await;
    insert_indicator_observation(&db, "demo-indicator", 1, 7).await;
    insert_indicator_observation(&db, "demo-indicator", 3, 13).await;
    let server = build_server(db);
    let definition_yaml = indoc::indoc! {"
        queries:
          metric:
            metric: indicator
            by: none
            indicator: demo-indicator
        viz:
          type: table
          columns:
            - metric
    "};

    let result = server
        .query_widget(strategy_id, definition_yaml)
        .await
        .expect("query indicator widget without daily bars");

    assert_eq!(
        result.as_json().clone(),
        json!({
            "as_of": "2099-01-03",
            "rows": [{
                "key": "market",
                "name": "Demo Indicator",
                "values": {"metric": 13}
            }]
        }),
    );
}

#[backend_test_macros::database_test]
async fn query_widget_rejects_amount_aggregation_with_unconfirmed_currency(
    db: gateway_postgres::DatabaseHandle,
) {
    let strategy_id = insert_strategy(&db, "widget-currency-error").await;
    insert_stock(&db, "US:DEMO-A", "Demo Foreign A").await;
    insert_stock(&db, "CA:TEST-B", "Demo Foreign B").await;
    insert_bars(&db, &["US:DEMO-A", "CA:TEST-B"]).await;
    let server = build_server(db);
    let definition_yaml = indoc::indoc! {"
        queries:
          close:
            metric: close
            by: none
            agg: sum
        viz:
          type: table
          columns:
            - close
        time:
          as_of: 2099-01-05
    "};

    let error = server
        .query_widget(strategy_id, definition_yaml)
        .await
        .expect_err("query amount aggregation with unconfirmed currency");

    assert_eq!(
        error,
        rmcp::ErrorData::invalid_params(
            "amount aggregation across multiple stocks requires confirmed currencies".to_string(),
            None,
        ),
    );
}
