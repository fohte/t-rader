use chrono::{NaiveDate, TimeZone, Utc};
use gateway_postgres::DatabaseHandle;
use gateway_postgres::entities::{bars, instruments, stock, valuation};
use rust_decimal::Decimal;
use sea_orm::{ActiveModelTrait, ActiveValue::Set, EntityTrait};
use serde_json::json;

use super::tests_common::{build_server, insert_strategy};

fn as_of() -> NaiveDate {
    NaiveDate::from_ymd_opt(2099, 1, 5).expect("valid test date")
}

async fn insert_stock(db: &DatabaseHandle) {
    stock::ActiveModel {
        id: Set("F001".to_string()),
        name: Set("Demo Stock".to_string()),
        market: Set(Some("OTHER".to_string())),
        product_category: Set(None),
        created_at: Default::default(),
        updated_at: Default::default(),
    }
    .insert(db)
    .await
    .expect("insert widget stock");
    instruments::Entity::insert(instruments::ActiveModel {
        id: Set("F001".to_string()),
        name: Set("Demo Stock".to_string()),
        market: Set("OTHER".to_string()),
        sector: Set(None),
    })
    .exec_without_returning(db)
    .await
    .expect("insert widget instrument");
}

async fn insert_market_data(db: &DatabaseHandle) {
    let timestamp = as_of()
        .and_hms_opt(0, 0, 0)
        .map(|timestamp| Utc.from_utc_datetime(&timestamp).fixed_offset())
        .expect("valid test timestamp");
    let close = Decimal::from(100);
    bars::Entity::insert(bars::ActiveModel {
        instrument_id: Set("F001".to_string()),
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
    insert_stock(&db).await;
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
