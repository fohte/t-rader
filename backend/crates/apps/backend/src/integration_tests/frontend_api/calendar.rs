use serde_json::json;

use crate::{
    integration_tests::calendar_test_support::{CalendarTestFixture, date},
    testing::create_test_server,
};

#[backend_test_macros::database_test]
async fn calendar_endpoint_keeps_targeted_earnings_and_summarizes_the_rest(
    db: gateway_postgres::DatabaseHandle,
) {
    let fixture = CalendarTestFixture::seed(&db).await;

    let server = create_test_server(db).await;
    let response = server
        .get(&format!(
            "/api/calendar/events?from={}&to={}&strategy_id={}",
            fixture.from, fixture.to, fixture.strategy_id,
        ))
        .await;

    assert_eq!(
        (response.status_code(), response.json::<serde_json::Value>(),),
        (
            axum::http::StatusCode::OK,
            json!({
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
        ),
    );
}

#[backend_test_macros::database_test]
async fn calendar_endpoint_rejects_reversed_date_range(db: gateway_postgres::DatabaseHandle) {
    let server = create_test_server(db).await;
    let response = server
        .get(&format!(
            "/api/calendar/events?from={}&to={}",
            date(2031, 2, 16),
            date(2031, 2, 10),
        ))
        .await;

    assert_eq!(
        (response.status_code(), response.json::<serde_json::Value>()),
        (
            axum::http::StatusCode::BAD_REQUEST,
            json!({ "error": "calendar event query range is invalid" }),
        ),
    );
}
