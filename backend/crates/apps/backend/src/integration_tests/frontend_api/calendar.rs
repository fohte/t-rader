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
                        "source": "jquants",
                        "external_id": "00010:2Q:2031-03-31",
                        "category": "earnings",
                        "country": "JP",
                        "title": "サンプル銘柄 A",
                        "stock_id": "0001",
                        "fiscal_period": "2031-03-31",
                        "fiscal_period_type": "2Q",
                        "event_date": fixture.event_date.to_string(),
                        "event_at": null,
                        "time_of_day": "pre_market",
                        "references": [
                            {
                                "kind": "individual_document",
                                "source": "sample_source",
                                "external_id": "document-00010:2Q:2031-03-31",
                                "title": "サンプル決算資料",
                                "url": "https://example.invalid/sample-document",
                                "target_period_start": null,
                                "target_period_end": "2031-03-31",
                                "target_period_type": "2Q",
                                "publication_evidence": "sample-release-record",
                                "publication_evidence_url": "https://example.invalid/sample-evidence",
                                "checked_at": "2023-11-14T22:13:20Z",
                            }
                        ],
                        "has_matching_earnings_summary": true,
                    },
                    {
                        "kind": "event",
                        "source": "jquants",
                        "external_id": "00020:2Q:2031-03-31",
                        "category": "earnings",
                        "country": "JP",
                        "title": "サンプル銘柄 B",
                        "stock_id": "0002",
                        "fiscal_period": "2031-03-31",
                        "fiscal_period_type": "2Q",
                        "event_date": fixture.event_date.to_string(),
                        "event_at": null,
                        "time_of_day": "pre_market",
                        "references": [],
                        "has_matching_earnings_summary": false,
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

    let expanded_response = server
        .get(&format!(
            "/api/calendar/other-earnings?event_date={}&country=JP&strategy_id={}",
            fixture.event_date, fixture.strategy_id,
        ))
        .await;

    assert_eq!(
        (
            expanded_response.status_code(),
            expanded_response.json::<serde_json::Value>()
        ),
        (
            axum::http::StatusCode::OK,
            json!({
                "events": [
                    {
                        "kind": "event",
                        "source": "jquants",
                        "external_id": "00030:2Q:2031-03-31",
                        "category": "earnings",
                        "country": "JP",
                        "title": "サンプル銘柄 C",
                        "stock_id": "0003",
                        "fiscal_period": "2031-03-31",
                        "fiscal_period_type": "2Q",
                        "event_date": fixture.event_date.to_string(),
                        "event_at": null,
                        "time_of_day": "pre_market",
                        "references": [
                            {
                                "kind": "individual_document",
                                "source": "sample_source",
                                "external_id": "document-00030:2Q:2031-03-31",
                                "title": "サンプル決算資料",
                                "url": "https://example.invalid/sample-document",
                                "target_period_start": null,
                                "target_period_end": "2031-03-31",
                                "target_period_type": "2Q",
                                "publication_evidence": "sample-release-record",
                                "publication_evidence_url": "https://example.invalid/sample-evidence",
                                "checked_at": "2023-11-14T22:13:20Z",
                            }
                        ],
                        "has_matching_earnings_summary": true,
                    }
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
