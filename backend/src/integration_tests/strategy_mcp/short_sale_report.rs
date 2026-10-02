use chrono::NaiveDate;
use core_application::short_sale_report::ShortSaleReportRepository;
use core_domain::short_sale_report::ShortSaleReport;
use sea_orm::{DatabaseBackend, DatabaseConnection, MockDatabase};
use serde_json::json;
use uuid::Uuid;

use super::StrategyServer;
use super::dto::ReadShortSaleReportsParams;
use super::tests_common::{build_server, insert_strategy};
use gateway_postgres::PostgresShortSaleReportRepository;

fn ymd(y: i32, m: u32, d: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, d).expect("valid date")
}

fn report(
    disc_date: NaiveDate,
    calc_date: NaiveDate,
    code: &str,
    reporter: &str,
    ratio: &str,
    previous: Option<(NaiveDate, &str)>,
) -> ShortSaleReport {
    ShortSaleReport {
        disc_date,
        calc_date,
        code: code.into(),
        ss_name: reporter.into(),
        ss_addr: String::new(),
        dic_name: String::new(),
        dic_addr: String::new(),
        fund_name: String::new(),
        short_position_ratio: ratio.parse().expect("valid ratio"),
        short_position_shares: 1_000,
        short_position_units: 10,
        prev_report_date: previous.map(|(date, _)| date),
        prev_report_ratio: previous.map(|(_, ratio)| ratio.parse().expect("valid ratio")),
        notes: String::new(),
    }
}

fn mock_db_with_strategy(strategy_id: Uuid) -> DatabaseConnection {
    let row =
        std::collections::BTreeMap::from([("id".to_string(), sea_orm::Value::from(strategy_id))]);
    MockDatabase::new(DatabaseBackend::Postgres)
        .append_query_results([vec![row]])
        .into_connection()
}

#[tokio::test]
async fn rejects_non_four_digit_symbol_through_tool_dispatch() {
    let strategy_id = Uuid::new_v4();
    let server = build_server(mock_db_with_strategy(strategy_id));

    let error = server
        .read_short_sale_reports(
            strategy_id,
            ReadShortSaleReportsParams {
                symbol: "00A0".into(),
                from: None,
                to: None,
                limit: None,
            },
        )
        .await
        .expect_err("non-numeric symbol should be rejected");

    assert_eq!(
        error,
        rmcp::ErrorData::invalid_params("symbol must be a 4-digit stock code, got \"00A0\"", None,),
    );
}

#[backend_test_macros::database_test]
async fn maps_matching_reports_and_preserves_query_range(db: gateway_postgres::DatabaseHandle) {
    PostgresShortSaleReportRepository::new(db.clone())
        .upsert(vec![
            report(
                ymd(2025, 3, 1),
                ymd(2025, 2, 26),
                "00001",
                "Synthetic A",
                "0.0153",
                Some((ymd(2025, 2, 19), "0.0102")),
            ),
            report(
                ymd(2025, 3, 2),
                ymd(2025, 2, 27),
                "00009",
                "Synthetic B",
                "0.0089",
                None,
            ),
            report(
                ymd(2025, 3, 2),
                ymd(2025, 2, 27),
                "99999",
                "Unrelated synthetic reporter",
                "0.0201",
                None,
            ),
        ])
        .await
        .expect("seed short sale reports");
    let strategy_id = insert_strategy(&db, "test-strategy").await;
    let server: StrategyServer = build_server(db);

    let result = server
        .read_short_sale_reports(
            strategy_id,
            ReadShortSaleReportsParams {
                symbol: "0000".into(),
                from: Some(ymd(2025, 3, 1)),
                to: Some(ymd(2025, 3, 2)),
                limit: Some(10),
            },
        )
        .await
        .expect("read short sale reports");

    assert_eq!(
        result.as_json().clone(),
        json!({
            "symbol": "0000",
            "items": [
                {
                    "disc_date": "2025-03-02",
                    "calc_date": "2025-02-27",
                    "reporter_name": "Synthetic B",
                    "reporter_address": null,
                    "client_name": null,
                    "client_address": null,
                    "fund_name": null,
                    "short_position_ratio": 0.0089,
                    "short_position_shares": 1000,
                    "short_position_units": 10,
                    "prev_report_date": null,
                    "prev_report_ratio": null,
                    "notes": null,
                },
                {
                    "disc_date": "2025-03-01",
                    "calc_date": "2025-02-26",
                    "reporter_name": "Synthetic A",
                    "reporter_address": null,
                    "client_name": null,
                    "client_address": null,
                    "fund_name": null,
                    "short_position_ratio": 0.0153,
                    "short_position_shares": 1000,
                    "short_position_units": 10,
                    "prev_report_date": "2025-02-19",
                    "prev_report_ratio": 0.0102,
                    "notes": null,
                },
            ],
        }),
    );
}

#[backend_test_macros::database_test]
async fn respects_limit_after_report_ordering(db: gateway_postgres::DatabaseHandle) {
    PostgresShortSaleReportRepository::new(db.clone())
        .upsert(vec![
            report(
                ymd(2025, 1, 1),
                ymd(2025, 1, 1),
                "00001",
                "Synthetic A",
                "0.01",
                None,
            ),
            report(
                ymd(2025, 1, 2),
                ymd(2025, 1, 2),
                "00001",
                "Synthetic B",
                "0.01",
                None,
            ),
            report(
                ymd(2025, 1, 3),
                ymd(2025, 1, 3),
                "00001",
                "Synthetic C",
                "0.01",
                None,
            ),
        ])
        .await
        .expect("seed short sale reports");
    let strategy_id = insert_strategy(&db, "test-strategy").await;
    let server: StrategyServer = build_server(db);

    let result = server
        .read_short_sale_reports(
            strategy_id,
            ReadShortSaleReportsParams {
                symbol: "0000".into(),
                from: None,
                to: None,
                limit: Some(2),
            },
        )
        .await
        .expect("read short sale reports");

    assert_eq!(
        result.as_json().clone(),
        json!({
            "symbol": "0000",
            "items": [
                {
                    "disc_date": "2025-01-03",
                    "calc_date": "2025-01-03",
                    "reporter_name": "Synthetic C",
                    "reporter_address": null,
                    "client_name": null,
                    "client_address": null,
                    "fund_name": null,
                    "short_position_ratio": 0.01,
                    "short_position_shares": 1000,
                    "short_position_units": 10,
                    "prev_report_date": null,
                    "prev_report_ratio": null,
                    "notes": null,
                },
                {
                    "disc_date": "2025-01-02",
                    "calc_date": "2025-01-02",
                    "reporter_name": "Synthetic B",
                    "reporter_address": null,
                    "client_name": null,
                    "client_address": null,
                    "fund_name": null,
                    "short_position_ratio": 0.01,
                    "short_position_shares": 1000,
                    "short_position_units": 10,
                    "prev_report_date": null,
                    "prev_report_ratio": null,
                    "notes": null,
                },
            ],
        }),
    );
}
