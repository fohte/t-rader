use serde_json::{Value, json};

use super::tests_common::build_server;
use crate::{
    integration_tests::calendar_test_support::{CalendarTestFixture, date},
    testing::insert_test_strategy,
};

#[backend_test_macros::database_test]
async fn read_calendar_keeps_targeted_earnings_and_summarizes_the_rest(
    db: gateway_postgres::DatabaseHandle,
) {
    let fixture = CalendarTestFixture::seed(&db).await;

    let server = build_server(db);
    let output = server
        .invoke::<_, Value>(
            "read_calendar",
            fixture.strategy_id,
            json!({ "from": fixture.from, "to": fixture.to }),
            None,
            None,
        )
        .await
        .expect("read calendar");

    assert_eq!(
        output.as_json(),
        &json!({
            "from": fixture.from.to_string(),
            "to": fixture.to.to_string(),
            "events": [
                {
                    "kind": "event",
                    "source": "sample_source",
                    "external_id": "tracked-direct",
                    "category": "earnings",
                    "country": "JP",
                    "title": "サンプル銘柄 A",
                    "stock_id": "0001",
                    "fiscal_period": "2031-03-31",
                    "event_date": fixture.event_date.to_string(),
                    "event_at": null,
                    "time_of_day": "pre_market",
                },
                {
                    "kind": "event",
                    "source": "sample_source",
                    "external_id": "tracked-group",
                    "category": "earnings",
                    "country": "JP",
                    "title": "サンプル銘柄 B",
                    "stock_id": "0002",
                    "fiscal_period": "2031-03-31",
                    "event_date": fixture.event_date.to_string(),
                    "event_at": null,
                    "time_of_day": "pre_market",
                },
                {
                    "kind": "other_earnings_summary",
                    "country": "JP",
                    "event_date": fixture.event_date.to_string(),
                    "count": 1,
                },
            ],
        }),
    );
}

#[backend_test_macros::database_test]
async fn read_calendar_rejects_reversed_date_range(db: gateway_postgres::DatabaseHandle) {
    let strategy_id = insert_test_strategy(&db, "sample-strategy").await;
    let error = build_server(db)
        .invoke::<_, Value>(
            "read_calendar",
            strategy_id,
            json!({
                "from": date(2031, 2, 16),
                "to": date(2031, 2, 10),
            }),
            None,
            None,
        )
        .await
        .expect_err("reversed date range should be rejected");

    assert_eq!(
        error,
        rmcp::ErrorData::invalid_params("calendar event query range is invalid", None),
    );
}
