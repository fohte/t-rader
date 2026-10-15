use chrono::{NaiveDate, TimeZone, Utc};
use core_application::valuation::ValuationRepository;
use core_domain::valuation::Valuation;
use gateway_postgres::PostgresValuationRepository;
use gateway_postgres::entities::{
    bars, financial_summary, group_axis, instruments, stock, stock_group, stock_group_member,
};
use rust_decimal::Decimal;
use sea_orm::ActiveModelTrait;
use sea_orm::ActiveValue::{NotSet, Set};
use serde_json::{Value, json};
use uuid::Uuid;

use super::test_server::StrategyServer;
use super::tests_common::{build_server, insert_strategy};

fn date(day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(2098, 1, day).expect("valid fixture date")
}

async fn seed_stock(db: &gateway_postgres::DatabaseHandle, id: &str, name: &str, market: &str) {
    stock::ActiveModel {
        id: Set(id.to_string()),
        name: Set(name.to_string()),
        market: Set(Some(market.to_string())),
        product_category: Set(None),
        created_at: NotSet,
        updated_at: NotSet,
    }
    .insert(db)
    .await
    .expect("insert screener stock");
}

async fn seed_sector_axis(db: &gateway_postgres::DatabaseHandle) -> (Uuid, Uuid, Uuid) {
    let axis_id = Uuid::new_v4();
    group_axis::ActiveModel {
        id: Set(axis_id),
        key: Set("demo-sector-axis".to_string()),
        name: Set("Demo sectors".to_string()),
        description: Set("Demo sector axis".to_string()),
        derive_from: Set(Some("tse_sector33".to_string())),
    }
    .insert(db)
    .await
    .expect("insert screener sector axis");

    let sector_ids = [
        ("sector-alpha", "Sector Alpha"),
        ("sector-beta", "Sector Beta"),
    ]
    .map(|(key, name)| (key.to_string(), name.to_string(), Uuid::new_v4()));
    for (key, name, id) in &sector_ids {
        stock_group::ActiveModel {
            id: Set(*id),
            axis_id: Set(axis_id),
            key: Set(key.clone()),
            name: Set(name.clone()),
            description: Set(None),
            code: Set(None),
        }
        .insert(db)
        .await
        .expect("insert screener sector");
    }

    (axis_id, sector_ids[0].2, sector_ids[1].2)
}

async fn seed_sector_membership(
    db: &gateway_postgres::DatabaseHandle,
    stock_id: &str,
    group_id: Uuid,
) {
    stock_group_member::ActiveModel {
        stock_id: Set(stock_id.to_string()),
        group_id: Set(group_id),
        created_at: NotSet,
    }
    .insert(db)
    .await
    .expect("insert screener sector membership");
}

async fn seed_daily_bar(db: &gateway_postgres::DatabaseHandle, id: &str, close: i64) {
    instruments::ActiveModel {
        id: Set(id.to_string()),
        name: Set(format!("Instrument {id}")),
        market: Set("TSE".to_string()),
        sector: Set(None),
    }
    .insert(db)
    .await
    .expect("insert screener instrument");

    let timestamp = Utc
        .from_utc_datetime(&date(3).and_hms_opt(0, 0, 0).expect("valid fixture time"))
        .fixed_offset();
    let close = Decimal::from(close);
    bars::ActiveModel {
        instrument_id: Set(id.to_string()),
        timeframe: Set("1d".to_string()),
        timestamp: Set(timestamp),
        open: Set(close),
        high: Set(close),
        low: Set(close),
        close: Set(close),
        volume: Set(100),
        adjustment_factor: Set(Decimal::ONE),
    }
    .insert(db)
    .await
    .expect("insert screener daily bar");
}

async fn seed_dividend_forecast(db: &gateway_postgres::DatabaseHandle, id: &str, forecast: f64) {
    financial_summary::ActiveModel {
        code: Set(format!("{id}0")),
        disclosure_no: Set("1".to_string()),
        disclosure_date: Set(date(2)),
        report_group_key: Set(format!("demo-report-{id}")),
        dividend_annual_forecast: Set(Some(forecast)),
        ..Default::default()
    }
    .insert(db)
    .await
    .expect("insert screener dividend forecast");
}

fn valuation(
    id: &str,
    day: u32,
    per: Option<Decimal>,
    pbr: Option<Decimal>,
    roe: Option<Decimal>,
    market_cap: Option<Decimal>,
) -> Valuation {
    Valuation {
        code: format!("{id}0"),
        date: date(day),
        eps: None,
        fwd_eps: None,
        bps: None,
        roe,
        fwd_roe: None,
        per,
        fwd_per: None,
        pbr,
        mkt_cap: market_cap,
    }
}

async fn seed_screener_stocks(db: &gateway_postgres::DatabaseHandle) {
    seed_stock(db, "D001", "Alpha Demo", "market-a").await;
    seed_stock(db, "D002", "Beta Demo", "market-b").await;
    seed_stock(db, "D003", "Gamma Demo", "market-b").await;
    seed_stock(db, "D004", "Missing Demo", "market-a").await;

    let (_, sector_alpha, sector_beta) = seed_sector_axis(db).await;
    seed_sector_membership(db, "D001", sector_alpha).await;
    seed_sector_membership(db, "D002", sector_beta).await;
    seed_sector_membership(db, "D003", sector_alpha).await;
    seed_daily_bar(db, "D001", 3250).await;
    seed_daily_bar(db, "D002", 1540).await;
    seed_daily_bar(db, "D003", 3000).await;
    seed_dividend_forecast(db, "D001", 123.5).await;
    seed_dividend_forecast(db, "D002", 52.36).await;
    seed_dividend_forecast(db, "D003", 114.0).await;

    PostgresValuationRepository::new(db.clone())
        .upsert(vec![
            valuation(
                "D001",
                1,
                Some(Decimal::new(82, 1)),
                Some(Decimal::new(7, 1)),
                Some(Decimal::new(91, 3)),
                Some(Decimal::from(512_000)),
            ),
            valuation(
                "D002",
                2,
                None,
                Some(Decimal::new(8, 1)),
                Some(Decimal::new(84, 3)),
                Some(Decimal::from(388_000)),
            ),
            valuation(
                "D003",
                3,
                Some(Decimal::new(83, 1)),
                Some(Decimal::new(7, 1)),
                Some(Decimal::new(92, 3)),
                Some(Decimal::from(510_000)),
            ),
        ])
        .await
        .expect("insert screener valuations");
}

#[backend_test_macros::database_test]
async fn screen_stocks_uses_screener_filters_and_preserves_missing_values(
    db: gateway_postgres::DatabaseHandle,
) {
    let strategy_id = insert_strategy(&db, "Demo strategy").await;
    seed_screener_stocks(&db).await;
    let server: StrategyServer = build_server(db);

    let filtered = server
        .invoke::<_, Value>(
            "screen_stocks",
            strategy_id,
            json!({
                "markets": ["market-a", "market-b"],
                "sectors": ["sector-alpha"],
                "min_per": 8,
                "max_per": 8.5,
                "min_pbr": 0.7,
                "max_pbr": 0.7,
                "min_roe": 9,
                "max_roe": 9.5,
                "min_dividend_yield": 3.7,
                "max_dividend_yield": 3.9,
                "min_market_cap": 5000,
                "max_market_cap": 5200,
                "sort_by": "market_cap",
                "sort_order": "desc",
            }),
            None,
            None,
        )
        .await
        .map(|result| result.as_json().clone());
    let unfiltered = server
        .invoke::<_, Value>("screen_stocks", strategy_id, json!({}), None, None)
        .await
        .map(|result| result.as_json().clone());

    assert_eq!(
        (filtered, unfiltered),
        (
            Ok(json!({
                "items": [
                    {
                        "id": "D001",
                        "name": "Alpha Demo",
                        "market": "market-a",
                        "sector": "Sector Alpha",
                        "close": 3250.0,
                        "per": 8.2,
                        "pbr": 0.7,
                        "roe": 9.1,
                        "dividend_yield": 3.8,
                        "market_cap": 5120.0,
                    },
                    {
                        "id": "D003",
                        "name": "Gamma Demo",
                        "market": "market-b",
                        "sector": "Sector Alpha",
                        "close": 3000.0,
                        "per": 8.3,
                        "pbr": 0.7,
                        "roe": 9.2,
                        "dividend_yield": 3.8,
                        "market_cap": 5100.0,
                    },
                ],
                "total_count": 2,
                "as_of": "2098-01-03",
                "page": 1,
            })),
            Ok(json!({
                "items": [
                    {
                        "id": "D001",
                        "name": "Alpha Demo",
                        "market": "market-a",
                        "sector": "Sector Alpha",
                        "close": 3250.0,
                        "per": 8.2,
                        "pbr": 0.7,
                        "roe": 9.1,
                        "dividend_yield": 3.8,
                        "market_cap": 5120.0,
                    },
                    {
                        "id": "D003",
                        "name": "Gamma Demo",
                        "market": "market-b",
                        "sector": "Sector Alpha",
                        "close": 3000.0,
                        "per": 8.3,
                        "pbr": 0.7,
                        "roe": 9.2,
                        "dividend_yield": 3.8,
                        "market_cap": 5100.0,
                    },
                    {
                        "id": "D002",
                        "name": "Beta Demo",
                        "market": "market-b",
                        "sector": "Sector Beta",
                        "close": 1540.0,
                        "per": null,
                        "pbr": 0.8,
                        "roe": 8.4,
                        "dividend_yield": 3.4,
                        "market_cap": 3880.0,
                    },
                    {
                        "id": "D004",
                        "name": "Missing Demo",
                        "market": "market-a",
                        "sector": null,
                        "close": null,
                        "per": null,
                        "pbr": null,
                        "roe": null,
                        "dividend_yield": null,
                        "market_cap": null,
                    },
                ],
                "total_count": 4,
                "as_of": "2098-01-03",
                "page": 1,
            })),
        ),
    );
}

#[backend_test_macros::database_test]
async fn screen_stocks_rejects_invalid_ranges_and_page(db: gateway_postgres::DatabaseHandle) {
    let strategy_id = insert_strategy(&db, "Demo strategy").await;
    let server: StrategyServer = build_server(db);

    let reversed_range = server
        .invoke::<_, Value>(
            "screen_stocks",
            strategy_id,
            json!({"min_per": 12, "max_per": 8}),
            None,
            None,
        )
        .await
        .map(|_| ());
    let zero_page = server
        .invoke::<_, Value>("screen_stocks", strategy_id, json!({"page": 0}), None, None)
        .await
        .map(|_| ());

    assert_eq!(
        (reversed_range, zero_page),
        (
            Err(rmcp::ErrorData::invalid_params(
                "per minimum must be less than or equal to maximum",
                None,
            )),
            Err(rmcp::ErrorData::invalid_params(
                "page must be greater than or equal to 1",
                None,
            )),
        ),
    );
}

fn expected_page(indices: impl Iterator<Item = u32>, page: u32) -> Value {
    let items = indices
        .map(|index| {
            json!({
                "id": format!("D{index:03}"),
                "name": format!("Demo {index:03}"),
                "market": "market-a",
                "sector": null,
                "close": null,
                "per": f64::from(index),
                "pbr": 1.0,
                "roe": 10.0,
                "dividend_yield": null,
                "market_cap": 1000.0 + f64::from(index),
            })
        })
        .collect::<Vec<_>>();

    json!({
        "items": items,
        "total_count": 101,
        "as_of": "2098-01-03",
        "page": page,
    })
}

#[backend_test_macros::database_test]
async fn screen_stocks_returns_hundred_rows_per_page(db: gateway_postgres::DatabaseHandle) {
    let strategy_id = insert_strategy(&db, "Demo strategy").await;
    let mut valuations = Vec::new();
    for index in 1..=101 {
        let id = format!("D{index:03}");
        seed_stock(&db, &id, &format!("Demo {index:03}"), "market-a").await;
        valuations.push(valuation(
            &id,
            3,
            Some(Decimal::from(index)),
            Some(Decimal::ONE),
            Some(Decimal::new(1, 1)),
            Some(Decimal::from(100_000 + index * 100)),
        ));
    }
    PostgresValuationRepository::new(db.clone())
        .upsert(valuations)
        .await
        .expect("insert screener page valuations");

    let server = build_server(db);
    let first_page = server
        .invoke::<_, Value>("screen_stocks", strategy_id, json!({}), None, None)
        .await
        .map(|result| result.as_json().clone());
    let second_page = server
        .invoke::<_, Value>("screen_stocks", strategy_id, json!({"page": 2}), None, None)
        .await
        .map(|result| result.as_json().clone());

    assert_eq!(
        (first_page, second_page),
        (
            Ok(expected_page((2..=101).rev(), 1)),
            Ok(expected_page(1..=1, 2)),
        ),
    );
}
