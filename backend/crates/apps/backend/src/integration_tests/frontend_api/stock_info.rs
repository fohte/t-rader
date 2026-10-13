use chrono::{Duration, NaiveDate, Utc};
use core_application::calendar::repository::CalendarEventRepository;
use core_application::unit_of_work::UnitOfWork;
use core_application::valuation::ValuationRepository;
use core_domain::calendar_event::{CalendarEvent, CalendarEventCategory, CalendarEventTimeOfDay};
use core_domain::valuation::Valuation;
use gateway_postgres::entities::financial_summary;
use gateway_postgres::{
    PostgresCalendarEventRepository, PostgresUnitOfWork, PostgresValuationRepository,
};
use rust_decimal::Decimal;
use sea_orm::{ActiveModelTrait, ActiveValue::Set};
use serde_json::{Value, json};

use crate::integration_tests::calendar_test_support::date;
use crate::testing::{create_test_server, insert_test_group_membership, insert_test_stock};

#[backend_test_macros::database_test]
async fn stock_groups_endpoint_returns_axis_and_group_keys_with_names(
    db: gateway_postgres::DatabaseHandle,
) {
    insert_test_stock(&db, "demo-code", "Demo Stock").await;
    insert_test_group_membership(
        &db,
        "demo-code",
        "demo-axis",
        "demo-group",
        "Demo Group",
        None,
    )
    .await;

    let server = create_test_server(db).await;
    let response = server.get("/api/stocks/demo-code/groups").await;

    assert_eq!(
        (response.status_code(), response.json::<Value>()),
        (
            axum::http::StatusCode::OK,
            json!([
                {
                    "axis": { "key": "demo-axis", "name": "Sample Axis" },
                    "group": { "key": "demo-group", "name": "Demo Group" },
                }
            ]),
        ),
    );
}

#[backend_test_macros::database_test]
async fn next_earnings_endpoint_returns_the_nearest_future_earnings_for_the_stock(
    db: gateway_postgres::DatabaseHandle,
) {
    let today = (Utc::now().naive_utc() + Duration::hours(9)).date();
    let next_date = today + Duration::days(2);
    let unit_of_work = PostgresUnitOfWork::new(db.clone());
    let repository = PostgresCalendarEventRepository::new(db.clone());
    let transaction = unit_of_work.begin().await.expect("begin transaction");
    repository
        .upsert(
            &transaction,
            vec![
                calendar_event(
                    "past",
                    "demo-code",
                    "Demo past earnings",
                    CalendarEventCategory::Earnings,
                    today - Duration::days(1),
                ),
                calendar_event(
                    "indicator",
                    "demo-code",
                    "Demo indicator",
                    CalendarEventCategory::Indicator,
                    today + Duration::days(1),
                ),
                calendar_event(
                    "other-stock",
                    "other-demo-code",
                    "Other demo earnings",
                    CalendarEventCategory::Earnings,
                    today + Duration::days(1),
                ),
                calendar_event(
                    "nearest",
                    "demo-code",
                    "Demo earnings",
                    CalendarEventCategory::Earnings,
                    next_date,
                ),
                calendar_event(
                    "later",
                    "demo-code",
                    "Later demo earnings",
                    CalendarEventCategory::Earnings,
                    today + Duration::days(5),
                ),
            ],
        )
        .await
        .expect("insert calendar events");
    unit_of_work
        .commit(transaction)
        .await
        .expect("commit calendar events");

    let server = create_test_server(db).await;
    let response = server.get("/api/stocks/demo-code/next-earnings").await;

    assert_eq!(
        (response.status_code(), response.json::<Value>(),),
        (
            axum::http::StatusCode::OK,
            json!({
                "title": "Demo earnings",
                "fiscal_period": "demo-period",
                "event_date": next_date.to_string(),
                "event_at": null,
                "time_of_day": "pre_market",
            }),
        ),
    );
}

#[backend_test_macros::database_test]
async fn next_earnings_endpoint_returns_null_when_no_future_earnings_exists(
    db: gateway_postgres::DatabaseHandle,
) {
    let server = create_test_server(db).await;
    let response = server
        .get("/api/stocks/unknown-demo-code/next-earnings")
        .await;

    assert_eq!(
        (response.status_code(), response.json::<Value>()),
        (axum::http::StatusCode::OK, Value::Null),
    );
}

#[backend_test_macros::database_test]
async fn valuation_endpoint_returns_the_latest_dated_value(db: gateway_postgres::DatabaseHandle) {
    let latest = Valuation {
        code: "DEMO".into(),
        date: date(2098, 1, 2),
        eps: Some(Decimal::new(125, 2)),
        fwd_eps: Some(Decimal::new(200, 2)),
        bps: Some(Decimal::new(500, 2)),
        roe: Some(Decimal::new(1200, 2)),
        fwd_roe: Some(Decimal::new(1400, 2)),
        per: Some(Decimal::new(1500, 2)),
        fwd_per: Some(Decimal::new(1200, 2)),
        pbr: Some(Decimal::new(130, 2)),
        mkt_cap: Some(Decimal::new(123456, 0)),
    };
    PostgresValuationRepository::new(db.clone())
        .upsert(vec![
            Valuation {
                code: "DEMO".into(),
                date: date(2098, 1, 1),
                eps: Some(Decimal::ONE),
                fwd_eps: None,
                bps: None,
                roe: None,
                fwd_roe: None,
                per: None,
                fwd_per: None,
                pbr: None,
                mkt_cap: None,
            },
            latest.clone(),
            Valuation {
                code: "OTHR".into(),
                date: date(2098, 1, 3),
                eps: None,
                fwd_eps: None,
                bps: None,
                roe: None,
                fwd_roe: None,
                per: None,
                fwd_per: None,
                pbr: None,
                mkt_cap: None,
            },
        ])
        .await
        .expect("insert valuations");

    let server = create_test_server(db).await;
    let response = server.get("/api/stocks/DEMO/valuation").await;

    assert_eq!(
        (response.status_code(), response.json::<Value>()),
        (
            axum::http::StatusCode::OK,
            json!({
                "code": latest.code,
                "date": latest.date.to_string(),
                "eps": latest.eps,
                "fwd_eps": latest.fwd_eps,
                "bps": latest.bps,
                "roe": latest.roe,
                "fwd_roe": latest.fwd_roe,
                "per": latest.per,
                "fwd_per": latest.fwd_per,
                "pbr": latest.pbr,
                "mkt_cap": latest.mkt_cap,
            }),
        ),
    );
}

#[backend_test_macros::database_test]
async fn valuation_endpoint_returns_null_when_the_stock_has_no_valuation(
    db: gateway_postgres::DatabaseHandle,
) {
    let server = create_test_server(db).await;
    let response = server.get("/api/stocks/DEMO/valuation").await;

    assert_eq!(
        (response.status_code(), response.json::<Value>()),
        (axum::http::StatusCode::OK, Value::Null),
    );
}

#[backend_test_macros::database_test]
async fn financial_summaries_endpoint_returns_complete_rows_in_newest_first_order(
    db: gateway_postgres::DatabaseHandle,
) {
    insert_financial_summary(&db, "001", date(2098, 1, 1), "demo-annual", 100.0, 10.0).await;
    insert_financial_summary(&db, "002", date(2098, 4, 1), "demo-quarter", 200.0, 20.0).await;

    let server = create_test_server(db).await;
    let response = server.get("/api/stocks/DEMO/financial-summaries").await;

    assert_eq!(
        (response.status_code(), response.json::<Value>()),
        (
            axum::http::StatusCode::OK,
            json!([
                financial_summary_json("002", date(2098, 4, 1), "demo-quarter", 200.0, 20.0),
                financial_summary_json("001", date(2098, 1, 1), "demo-annual", 100.0, 10.0),
            ]),
        ),
    );
}

async fn insert_financial_summary(
    db: &gateway_postgres::DatabaseHandle,
    disclosure_no: &str,
    disclosure_date: NaiveDate,
    report_group_key: &str,
    sales: f64,
    eps: f64,
) {
    financial_summary::ActiveModel {
        code: Set("DEMO".into()),
        disclosure_no: Set(disclosure_no.into()),
        disclosure_date: Set(disclosure_date),
        report_group_key: Set(report_group_key.into()),
        sales: Set(Some(sales)),
        eps: Set(Some(eps)),
        ..Default::default()
    }
    .insert(db)
    .await
    .expect("insert financial summary");
}

fn financial_summary_json(
    disclosure_no: &str,
    disclosure_date: NaiveDate,
    report_group_key: &str,
    sales: f64,
    eps: f64,
) -> Value {
    json!({
        "code": "DEMO",
        "disclosure_no": disclosure_no,
        "disclosure_date": disclosure_date.to_string(),
        "report_group_key": report_group_key,
        "document_type": null,
        "current_period_type": null,
        "current_period_start": null,
        "current_period_end": null,
        "current_fiscal_year_start": null,
        "current_fiscal_year_end": null,
        "sales": sales,
        "operating_profit": null,
        "ordinary_profit": null,
        "net_profit": null,
        "eps": eps,
        "bps": null,
        "total_assets": null,
        "equity": null,
        "equity_to_asset_ratio": null,
        "roe": null,
        "cash_flow_operating": null,
        "cash_flow_investing": null,
        "cash_flow_financing": null,
        "cash_and_equivalents": null,
        "dividend_annual": null,
        "dividend_annual_forecast": null,
        "dividend_annual_forecast_next": null,
        "forecast_sales": null,
        "forecast_operating_profit": null,
        "forecast_ordinary_profit": null,
        "forecast_net_profit": null,
        "forecast_eps": null,
        "next_forecast_sales": null,
        "next_forecast_operating_profit": null,
        "next_forecast_ordinary_profit": null,
        "next_forecast_net_profit": null,
        "next_forecast_eps": null,
    })
}

fn calendar_event(
    external_id: &str,
    stock_id: &str,
    title: &str,
    category: CalendarEventCategory,
    event_date: NaiveDate,
) -> CalendarEvent {
    CalendarEvent {
        source: "demo-source".into(),
        external_id: external_id.into(),
        category,
        country: "ZZ".into(),
        title: title.into(),
        stock_id: Some(stock_id.into()),
        fiscal_period: Some("demo-period".into()),
        event_date,
        event_at: None,
        time_of_day: Some(CalendarEventTimeOfDay::PreMarket),
    }
}
