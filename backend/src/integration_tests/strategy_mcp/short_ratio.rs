use chrono::NaiveDate;
use core_application::short_ratio::ShortRatioRepository;
use core_domain::short_ratio::ShortRatio;
use rust_decimal::Decimal;
use serde_json::json;
use uuid::Uuid;

use super::StrategyServer;
use super::dto::ReadSectorShortRatioParams;
use super::tests_common::{build_server, insert_strategy, mock_db_with_strategy};
use gateway_postgres::PostgresShortRatioRepository;

fn ymd(y: i32, m: u32, d: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, d).expect("valid date")
}

fn dec(s: &str) -> Decimal {
    s.parse().expect("valid decimal")
}

fn ratio(date: NaiveDate, sector33_code: &str, values: Option<(&str, &str, &str)>) -> ShortRatio {
    let (sell_excluding_short_value, short_with_restriction_value, short_without_restriction_value) =
        values.map_or((None, None, None), |(sell, with_r, without_r)| {
            (Some(dec(sell)), Some(dec(with_r)), Some(dec(without_r)))
        });
    ShortRatio {
        date,
        sector33_code: sector33_code.into(),
        sell_excluding_short_value,
        short_with_restriction_value,
        short_without_restriction_value,
    }
}

#[tokio::test]
async fn rejects_unknown_group_key_through_tool_dispatch() {
    let strategy_id = Uuid::new_v4();
    let server = build_server(mock_db_with_strategy(strategy_id));

    let error = server
        .read_sector_short_ratio(
            strategy_id,
            ReadSectorShortRatioParams {
                sector: "合成業種".into(),
                from: None,
                to: None,
                limit: None,
            },
        )
        .await
        .expect_err("unknown group key should be rejected");

    assert_eq!(
        error,
        rmcp::ErrorData::invalid_params("unknown J-Quants industry group key: \"合成業種\"", None,),
    );
}

#[backend_test_macros::database_test]
async fn reads_sector_rows_and_maps_values_to_tool_json(db: gateway_postgres::DatabaseHandle) {
    PostgresShortRatioRepository::new(db.clone())
        .upsert(vec![
            ratio(ymd(2025, 1, 5), "9999", Some(("700", "200", "100"))),
            ratio(ymd(2025, 1, 6), "9999", None),
            ratio(ymd(2025, 1, 6), "9050", Some(("100", "50", "50"))),
        ])
        .await
        .expect("seed short ratios");
    let strategy_id = insert_strategy(&db, "test-strategy").await;
    let server: StrategyServer = build_server(db);

    let result = server
        .read_sector_short_ratio(
            strategy_id,
            ReadSectorShortRatioParams {
                sector: "その他".into(),
                from: Some(ymd(2025, 1, 5)),
                to: Some(ymd(2025, 1, 6)),
                limit: Some(5),
            },
        )
        .await
        .expect("read short ratio");

    assert_eq!(
        result.as_json().clone(),
        json!({
            "sector": "その他",
            "items": [
                {
                    "date": "2025-01-06",
                    "sell_excluding_short_value": null,
                    "short_with_restriction_value": null,
                    "short_without_restriction_value": null,
                    "short_ratio": null,
                },
                {
                    "date": "2025-01-05",
                    "sell_excluding_short_value": 700.0,
                    "short_with_restriction_value": 200.0,
                    "short_without_restriction_value": 100.0,
                    "short_ratio": 0.3,
                },
            ],
        }),
    );
}
