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
use sea_orm::EntityTrait;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::testing::create_test_server;

fn normalize_result(mut result: Value) -> Value {
    for stock in result["items"].as_array_mut().expect("result items") {
        for field in ["close", "per", "pbr", "roe", "dividend_yield", "market_cap"] {
            if let Some(serde_json::Value::Number(value)) = stock.get(field) {
                let decimal = value
                    .to_string()
                    .parse::<Decimal>()
                    .expect("numeric result field");
                stock[field] = serde_json::from_str(&decimal.normalize().to_string())
                    .expect("normalized numeric result field");
            }
        }
    }
    result
}

fn date(day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(2098, 1, day).expect("valid test date")
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

async fn seed_instrument(db: &gateway_postgres::DatabaseHandle, id: &str) {
    instruments::Entity::insert(instruments::ActiveModel {
        id: Set(id.to_string()),
        name: Set(format!("Instrument {id}")),
        market: Set("TSE".to_string()),
        sector: Set(None),
    })
    .exec_without_returning(db)
    .await
    .expect("insert screener instrument");
}

async fn seed_daily_bar(db: &gateway_postgres::DatabaseHandle, id: &str, day: u32, close: i64) {
    let timestamp = date(day)
        .and_hms_opt(0, 0, 0)
        .map(|datetime| Utc.from_utc_datetime(&datetime).fixed_offset())
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
    .expect("insert screener daily bar");
}

async fn seed_financial_summary(
    db: &gateway_postgres::DatabaseHandle,
    code: &str,
    disclosure_no: &str,
    disclosure_day: u32,
    dividend_forecast: f64,
) {
    financial_summary::ActiveModel {
        code: Set(code.to_string()),
        disclosure_no: Set(disclosure_no.to_string()),
        disclosure_date: Set(date(disclosure_day)),
        report_group_key: Set(format!("demo-report-{code}-{disclosure_no}")),
        dividend_annual_forecast: Set(Some(dividend_forecast)),
        ..Default::default()
    }
    .insert(db)
    .await
    .expect("insert screener financial summary");
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

async fn seed_sector_axis(db: &gateway_postgres::DatabaseHandle) -> (Uuid, Uuid, Uuid) {
    let axis_id = Uuid::new_v4();
    group_axis::ActiveModel {
        id: Set(axis_id),
        key: Set("demo-tse-sector-axis".to_string()),
        name: Set("Demo TSE sectors".to_string()),
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

async fn seed_screener_data(db: &gateway_postgres::DatabaseHandle) {
    seed_stock(db, "D001", "Alpha Demo", "market-a").await;
    seed_stock(db, "D002", "Beta Demo", "market-b").await;
    seed_stock(db, "D003", "Gamma Demo", "market-b").await;
    seed_stock(db, "D004", "Missing Demo", "market-a").await;

    let (_, sector_alpha, sector_beta) = seed_sector_axis(db).await;
    seed_sector_membership(db, "D001", sector_alpha).await;
    seed_sector_membership(db, "D002", sector_beta).await;
    seed_sector_membership(db, "D003", sector_alpha).await;

    PostgresValuationRepository::new(db.clone())
        .upsert(vec![
            valuation(
                "D001",
                1,
                Some(Decimal::from(99)),
                Some(Decimal::from(9)),
                Some(Decimal::new(10, 2)),
                Some(Decimal::from(999_900)),
            ),
            valuation(
                "D001",
                3,
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

    for id in ["D001", "D002", "D003"] {
        seed_instrument(db, id).await;
    }
    seed_daily_bar(db, "D001", 2, 3000).await;
    seed_daily_bar(db, "D001", 3, 3250).await;
    seed_daily_bar(db, "D002", 3, 1540).await;
    seed_daily_bar(db, "D003", 3, 3000).await;

    seed_financial_summary(db, "D0010", "1", 1, 50.0).await;
    seed_financial_summary(db, "D0010", "2", 2, 123.5).await;
    seed_financial_summary(db, "D0020", "1", 2, 52.36).await;
    seed_financial_summary(db, "D0030", "1", 2, 114.0).await;
}

#[backend_test_macros::database_test]
async fn screener_endpoint_filters_markets_sectors_and_metric_ranges(
    db: gateway_postgres::DatabaseHandle,
) {
    seed_screener_data(&db).await;
    let server = create_test_server(db).await;
    let response = server
        .get("/api/screener?markets=market-a,market-b&sectors=sector-alpha&min_per=8&max_per=8.5&min_pbr=0.7&max_pbr=0.7&min_roe=9&max_roe=9.5&min_dividend_yield=3.7&max_dividend_yield=3.9&min_market_cap=5000&max_market_cap=5200&sort_by=market_cap&sort_order=desc")
        .await;

    assert_eq!(
        (
            response.status_code(),
            normalize_result(response.json::<Value>())
        ),
        (
            axum::http::StatusCode::OK,
            json!({
                "items": [
                    {
                        "id": "D001",
                        "name": "Alpha Demo",
                        "market": "market-a",
                        "sector": "Sector Alpha",
                        "close": 3250,
                        "per": 8.2,
                        "pbr": 0.7,
                        "roe": 9.1,
                        "dividend_yield": 3.8,
                        "market_cap": 5120,
                    },
                    {
                        "id": "D003",
                        "name": "Gamma Demo",
                        "market": "market-b",
                        "sector": "Sector Alpha",
                        "close": 3000,
                        "per": 8.3,
                        "pbr": 0.7,
                        "roe": 9.2,
                        "dividend_yield": 3.8,
                        "market_cap": 5100,
                    },
                ],
                "total_count": 2,
                "as_of": "2098-01-03",
                "page": 1,
            }),
        ),
    );
}

#[backend_test_macros::database_test]
async fn screener_endpoint_keeps_missing_values_unless_their_metric_is_filtered(
    db: gateway_postgres::DatabaseHandle,
) {
    seed_screener_data(&db).await;
    let server = create_test_server(db).await;
    let all = server.get("/api/screener").await;
    let filtered = server.get("/api/screener?min_per=0").await;
    let all_body = normalize_result(all.json());
    let filtered_body = normalize_result(filtered.json());

    assert_eq!(
        (
            all.status_code(),
            all_body,
            filtered.status_code(),
            filtered_body,
        ),
        (
            axum::http::StatusCode::OK,
            json!({
                "items": [
                    {
                        "id": "D001",
                        "name": "Alpha Demo",
                        "market": "market-a",
                        "sector": "Sector Alpha",
                        "close": 3250,
                        "per": 8.2,
                        "pbr": 0.7,
                        "roe": 9.1,
                        "dividend_yield": 3.8,
                        "market_cap": 5120,
                    },
                    {
                        "id": "D003",
                        "name": "Gamma Demo",
                        "market": "market-b",
                        "sector": "Sector Alpha",
                        "close": 3000,
                        "per": 8.3,
                        "pbr": 0.7,
                        "roe": 9.2,
                        "dividend_yield": 3.8,
                        "market_cap": 5100,
                    },
                    {
                        "id": "D002",
                        "name": "Beta Demo",
                        "market": "market-b",
                        "sector": "Sector Beta",
                        "close": 1540,
                        "per": null,
                        "pbr": 0.8,
                        "roe": 8.4,
                        "dividend_yield": 3.4,
                        "market_cap": 3880,
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
            }),
            axum::http::StatusCode::OK,
            json!({
                "items": [
                    {
                        "id": "D001",
                        "name": "Alpha Demo",
                        "market": "market-a",
                        "sector": "Sector Alpha",
                        "close": 3250,
                        "per": 8.2,
                        "pbr": 0.7,
                        "roe": 9.1,
                        "dividend_yield": 3.8,
                        "market_cap": 5120,
                    },
                    {
                        "id": "D003",
                        "name": "Gamma Demo",
                        "market": "market-b",
                        "sector": "Sector Alpha",
                        "close": 3000,
                        "per": 8.3,
                        "pbr": 0.7,
                        "roe": 9.2,
                        "dividend_yield": 3.8,
                        "market_cap": 5100,
                    },
                ],
                "total_count": 2,
                "as_of": "2098-01-03",
                "page": 1,
            }),
        ),
    );
}

#[backend_test_macros::database_test]
async fn screener_endpoint_rejects_reversed_metric_ranges(db: gateway_postgres::DatabaseHandle) {
    let server = create_test_server(db).await;
    let response = server.get("/api/screener?min_per=12&max_per=8").await;

    assert_eq!(
        (response.status_code(), response.json::<Value>()),
        (
            axum::http::StatusCode::BAD_REQUEST,
            json!({ "error": "per minimum must be less than or equal to maximum" }),
        ),
    );
}

#[backend_test_macros::database_test]
async fn screener_endpoint_sorts_and_returns_one_hundred_rows_per_page(
    db: gateway_postgres::DatabaseHandle,
) {
    let mut valuations = Vec::new();
    for index in 1..=101 {
        let id = format!("D{index:03}");
        seed_stock(&db, &id, &format!("Demo {index:03}"), "market-a").await;
        valuations.push(valuation(
            &id,
            3,
            Some(Decimal::from(index)),
            Some(Decimal::ONE),
            Some(Decimal::new(1, 2)),
            Some(Decimal::from(100_000 + index * 100)),
        ));
    }
    PostgresValuationRepository::new(db.clone())
        .upsert(valuations)
        .await
        .expect("insert screener page valuations");

    let server = create_test_server(db).await;
    let first_page = server
        .get("/api/screener?sort_by=market_cap&sort_order=desc")
        .await;
    let second_page = server
        .get("/api/screener?sort_by=market_cap&sort_order=desc&page=2")
        .await;
    let first_body: Value = first_page.json();
    let second_body: Value = second_page.json();
    let first_ids = first_body["items"]
        .as_array()
        .expect("first page items")
        .iter()
        .map(|row| row["id"].as_str().expect("stock id").to_string())
        .collect::<Vec<_>>();
    let second_ids = second_body["items"]
        .as_array()
        .expect("second page items")
        .iter()
        .map(|row| row["id"].as_str().expect("stock id").to_string())
        .collect::<Vec<_>>();
    let expected_first_ids = (2..=101)
        .rev()
        .map(|index| format!("D{index:03}"))
        .collect::<Vec<_>>();

    assert_eq!(
        (
            first_page.status_code(),
            first_ids,
            first_body["total_count"].clone(),
            first_body["as_of"].clone(),
            first_body["page"].clone(),
            second_page.status_code(),
            second_ids,
            second_body["total_count"].clone(),
            second_body["as_of"].clone(),
            second_body["page"].clone(),
        ),
        (
            axum::http::StatusCode::OK,
            expected_first_ids,
            json!(101),
            json!("2098-01-03"),
            json!(1),
            axum::http::StatusCode::OK,
            vec!["D001".to_string()],
            json!(101),
            json!("2098-01-03"),
            json!(2),
        ),
    );
}
