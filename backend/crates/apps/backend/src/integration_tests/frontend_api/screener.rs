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
    let Some(items) = result["items"].as_array_mut() else {
        panic!("unexpected screener response: {result}");
    };
    for stock in items {
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

fn per_for_index(index: u32) -> Option<Decimal> {
    (index != 1).then(|| Decimal::from(index))
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
                "per": per_for_index(index)
                    .map(|per| json!(per))
                    .unwrap_or(Value::Null),
                "pbr": 1,
                "roe": 1,
                "dividend_yield": null,
                "market_cap": 1000 + index,
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

fn date(day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(2098, 1, day).expect("valid test date")
}

fn days_before_latest(days: i64) -> NaiveDate {
    date(3) - chrono::Duration::days(days)
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
    seed_daily_bar_at(db, id, date(day), close).await;
}

async fn seed_daily_bar_at(
    db: &gateway_postgres::DatabaseHandle,
    id: &str,
    day: NaiveDate,
    close: i64,
) {
    let timestamp = day
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
    seed_financial_summary_row(
        db,
        code,
        disclosure_no,
        disclosure_day,
        None,
        Some(dividend_forecast),
        None,
    )
    .await;
}

async fn seed_financial_summary_row(
    db: &gateway_postgres::DatabaseHandle,
    code: &str,
    disclosure_no: &str,
    disclosure_day: u32,
    current_period_type: Option<&str>,
    dividend_forecast: Option<f64>,
    dividend_forecast_next: Option<f64>,
) {
    financial_summary::ActiveModel {
        code: Set(code.to_string()),
        disclosure_no: Set(disclosure_no.to_string()),
        disclosure_date: Set(date(disclosure_day)),
        report_group_key: Set(format!("demo-report-{code}-{disclosure_no}")),
        current_period_type: Set(current_period_type.map(str::to_string)),
        dividend_annual_forecast: Set(dividend_forecast),
        dividend_annual_forecast_next: Set(dividend_forecast_next),
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
    valuation_at(id, date(day), per, pbr, roe, market_cap)
}

fn valuation_at(
    id: &str,
    valuation_date: NaiveDate,
    per: Option<Decimal>,
    pbr: Option<Decimal>,
    roe: Option<Decimal>,
    market_cap: Option<Decimal>,
) -> Valuation {
    Valuation {
        code: format!("{id}0"),
        date: valuation_date,
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
        ("sector-alpha", "Sector Alpha", "2000"),
        ("sector-beta", "Sector Beta", "1000"),
    ]
    .map(|(key, name, code)| {
        (
            key.to_string(),
            name.to_string(),
            code.to_string(),
            Uuid::new_v4(),
        )
    });
    for (key, name, code, id) in &sector_ids {
        stock_group::ActiveModel {
            id: Set(*id),
            axis_id: Set(axis_id),
            key: Set(key.clone()),
            name: Set(name.clone()),
            description: Set(None),
            code: Set(Some(code.clone())),
        }
        .insert(db)
        .await
        .expect("insert screener sector");
    }

    (axis_id, sector_ids[0].3, sector_ids[1].3)
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
async fn screener_sector_options_only_include_tse_sector33_groups(
    db: gateway_postgres::DatabaseHandle,
) {
    seed_screener_data(&db).await;

    let axis_id = Uuid::new_v4();
    group_axis::ActiveModel {
        id: Set(axis_id),
        key: Set("demo-custom-axis".to_string()),
        name: Set("Demo custom groups".to_string()),
        description: Set("Custom axis".to_string()),
        derive_from: Set(None),
    }
    .insert(&db)
    .await
    .expect("insert unrelated group axis");
    stock_group::ActiveModel {
        id: Set(Uuid::new_v4()),
        axis_id: Set(axis_id),
        key: Set("unrelated-group".to_string()),
        name: Set("Unrelated Group".to_string()),
        description: Set(None),
        code: Set(None),
    }
    .insert(&db)
    .await
    .expect("insert unrelated group");

    let server = create_test_server(db).await;
    let response = server.get("/api/screener/sectors").await;

    assert_eq!(
        (response.status_code(), response.json::<Value>()),
        (
            axum::http::StatusCode::OK,
            json!([
                { "key": "sector-beta", "name": "Sector Beta" },
                { "key": "sector-alpha", "name": "Sector Alpha" },
            ]),
        ),
    );
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
async fn screener_endpoint_limits_latest_metrics_to_thirty_days(
    db: gateway_postgres::DatabaseHandle,
) {
    seed_stock(&db, "D901", "Recent Demo", "market-a").await;
    seed_stock(&db, "D902", "Boundary Demo", "market-a").await;
    seed_stock(&db, "D903", "Stale Demo", "market-a").await;
    for id in ["D901", "D902", "D903"] {
        seed_instrument(&db, id).await;
    }
    seed_daily_bar_at(&db, "D901", date(3), 1200).await;
    seed_daily_bar_at(&db, "D902", days_before_latest(30), 2400).await;
    seed_daily_bar_at(&db, "D903", days_before_latest(31), 3600).await;
    PostgresValuationRepository::new(db.clone())
        .upsert(vec![
            valuation_at(
                "D901",
                date(3),
                Some(Decimal::from(10)),
                Some(Decimal::ONE),
                Some(Decimal::new(1, 1)),
                Some(Decimal::from(100_000)),
            ),
            valuation_at(
                "D902",
                days_before_latest(30),
                Some(Decimal::from(20)),
                Some(Decimal::from(2)),
                Some(Decimal::new(2, 1)),
                Some(Decimal::from(200_000)),
            ),
            valuation_at(
                "D903",
                days_before_latest(31),
                Some(Decimal::from(30)),
                Some(Decimal::from(3)),
                Some(Decimal::new(3, 1)),
                Some(Decimal::from(300_000)),
            ),
        ])
        .await
        .expect("insert screener window valuations");

    let server = create_test_server(db).await;
    let response = server.get("/api/screener").await;

    assert_eq!(
        (
            response.status_code(),
            normalize_result(response.json::<Value>()),
        ),
        (
            axum::http::StatusCode::OK,
            json!({
                "items": [
                    {
                        "id": "D902",
                        "name": "Boundary Demo",
                        "market": "market-a",
                        "sector": null,
                        "close": 2400,
                        "per": 20,
                        "pbr": 2,
                        "roe": 20,
                        "dividend_yield": null,
                        "market_cap": 2000,
                    },
                    {
                        "id": "D901",
                        "name": "Recent Demo",
                        "market": "market-a",
                        "sector": null,
                        "close": 1200,
                        "per": 10,
                        "pbr": 1,
                        "roe": 10,
                        "dividend_yield": null,
                        "market_cap": 1000,
                    },
                    {
                        "id": "D903",
                        "name": "Stale Demo",
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
                "total_count": 3,
                "as_of": "2098-01-03",
                "page": 1,
            }),
        ),
    );
}

#[backend_test_macros::database_test]
async fn screener_endpoint_preserves_as_of_when_all_valuations_are_outside_the_window(
    db: gateway_postgres::DatabaseHandle,
) {
    seed_stock(&db, "D904", "Stale Demo", "market-a").await;
    seed_stock(&db, "D905", "Recent Demo", "market-a").await;
    seed_instrument(&db, "D904").await;
    seed_instrument(&db, "D905").await;
    seed_daily_bar_at(&db, "D904", days_before_latest(31), 3600).await;
    seed_daily_bar_at(&db, "D905", date(3), 1200).await;
    PostgresValuationRepository::new(db.clone())
        .upsert(vec![valuation_at(
            "D904",
            days_before_latest(31),
            Some(Decimal::from(30)),
            Some(Decimal::from(3)),
            Some(Decimal::new(3, 1)),
            Some(Decimal::from(300_000)),
        )])
        .await
        .expect("insert screener stale valuation");

    let server = create_test_server(db).await;
    let response = server.get("/api/screener").await;

    assert_eq!(
        (
            response.status_code(),
            normalize_result(response.json::<Value>()),
        ),
        (
            axum::http::StatusCode::OK,
            json!({
                "items": [
                    {
                        "id": "D904",
                        "name": "Stale Demo",
                        "market": "market-a",
                        "sector": null,
                        "close": null,
                        "per": null,
                        "pbr": null,
                        "roe": null,
                        "dividend_yield": null,
                        "market_cap": null,
                    },
                    {
                        "id": "D905",
                        "name": "Recent Demo",
                        "market": "market-a",
                        "sector": null,
                        "close": 1200,
                        "per": null,
                        "pbr": null,
                        "roe": null,
                        "dividend_yield": null,
                        "market_cap": null,
                    },
                ],
                "total_count": 2,
                "as_of": "2097-12-03",
                "page": 1,
            }),
        ),
    );
}

#[backend_test_macros::database_test]
async fn screener_endpoint_uses_next_year_forecast_after_fy_disclosures(
    db: gateway_postgres::DatabaseHandle,
) {
    seed_stock(&db, "D005", "Delta Demo", "market-a").await;
    seed_instrument(&db, "D005").await;
    seed_daily_bar(&db, "D005", 3, 2000).await;
    seed_financial_summary_row(&db, "D0050", "1", 1, Some("3Q"), Some(60.0), None).await;
    seed_financial_summary_row(&db, "D0050", "2", 2, Some("FY"), None, Some(80.0)).await;
    seed_financial_summary_row(&db, "D0050", "3", 3, Some("FY"), None, None).await;

    let server = create_test_server(db).await;
    let response = server.get("/api/screener?min_dividend_yield=3.9").await;

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
                        "id": "D005",
                        "name": "Delta Demo",
                        "market": "market-a",
                        "sector": null,
                        "close": 2000,
                        "per": null,
                        "pbr": null,
                        "roe": null,
                        "dividend_yield": 4,
                        "market_cap": null,
                    },
                ],
                "total_count": 1,
                "as_of": null,
                "page": 1,
            }),
        ),
    );
}

#[backend_test_macros::database_test]
async fn screener_endpoint_rejects_invalid_queries(db: gateway_postgres::DatabaseHandle) {
    let server = create_test_server(db).await;
    let reversed_range = server.get("/api/screener?min_per=12&max_per=8").await;
    let zero_page = server.get("/api/screener?page=0").await;

    assert_eq!(
        (
            (reversed_range.status_code(), reversed_range.json::<Value>()),
            (zero_page.status_code(), zero_page.json::<Value>()),
        ),
        (
            (
                axum::http::StatusCode::BAD_REQUEST,
                json!({ "error": "per minimum must be less than or equal to maximum" }),
            ),
            (
                axum::http::StatusCode::BAD_REQUEST,
                json!({ "error": "page must be greater than or equal to 1" }),
            ),
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
            per_for_index(index),
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
    let first_per_ascending_page = server.get("/api/screener?sort_by=per&sort_order=asc").await;
    let second_per_ascending_page = server
        .get("/api/screener?sort_by=per&sort_order=asc&page=2")
        .await;
    let first_body = normalize_result(first_page.json::<Value>());
    let second_body = normalize_result(second_page.json::<Value>());
    let first_per_ascending_body = normalize_result(first_per_ascending_page.json::<Value>());
    let second_per_ascending_body = normalize_result(second_per_ascending_page.json::<Value>());

    assert_eq!(
        (
            first_page.status_code(),
            first_body,
            second_page.status_code(),
            second_body,
        ),
        (
            axum::http::StatusCode::OK,
            expected_page((2..=101).rev(), 1),
            axum::http::StatusCode::OK,
            expected_page(1..=1, 2),
        ),
    );

    assert_eq!(
        (
            first_per_ascending_page.status_code(),
            first_per_ascending_body,
            second_per_ascending_page.status_code(),
            second_per_ascending_body,
        ),
        (
            axum::http::StatusCode::OK,
            expected_page(2..=101, 1),
            axum::http::StatusCode::OK,
            expected_page(1..=1, 2),
        ),
    );
}
