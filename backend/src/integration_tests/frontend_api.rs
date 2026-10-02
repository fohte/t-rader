fn assert_response_eq(
    response: &axum_test::TestResponse,
    expected_status: axum::http::StatusCode,
    expected_body: Option<serde_json::Value>,
) {
    let actual_body = if response.as_bytes().is_empty() {
        None
    } else {
        Some(response.json::<serde_json::Value>())
    };

    assert_eq!(
        (response.status_code(), actual_body),
        (expected_status, expected_body),
    );
}

mod agent_config;
mod agent_options;
mod annotations;
mod bars;
mod comments;
mod config;
mod custom_indicators;
mod group_axes;
mod history;
mod ingest_status;
mod note_kinds;
mod note_predictions;
mod note_versions;
mod notes;
mod refs;
mod risk_policy;
mod rss_feeds;
mod strategies;
mod strategy_investable_amount;
mod strategy_tasks;
mod tasks;
mod trade_notes;
mod trades;
mod triggers;
