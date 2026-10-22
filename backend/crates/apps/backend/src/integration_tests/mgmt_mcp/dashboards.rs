use std::sync::Arc;

use async_trait::async_trait;
use chrono::NaiveDate;
use core_application::widget::{
    SharedWidgetGroupMembershipSource, WidgetGroupMembershipSource, WidgetMetric, WidgetQuery,
    WidgetQueryError, WidgetQueryRow, WidgetQuerySource, WidgetRowKey, WidgetUseCases,
};
use indoc::indoc;
use rust_decimal::Decimal;
use serde_json::json;

use core_application::agent_task_client::FakeAgentTaskClient;

use super::tests_common::build_server;

const DASHBOARD_YAML: &str = indoc! {"
    title: sample-dashboard
    widgets:
      - title: sample-widget
        width: full
        queries:
          movement:
            metric: change
            period: 1d
            by: none
            agg: mean
        viz:
          type: toplist
          value: movement
          order: desc
          limit: 1
"};

struct FakeWidgetQuerySource {
    unavailable_date: Option<NaiveDate>,
    incompatible_metric: Option<WidgetMetric>,
}

struct EmptyWidgetGroupMembershipSource;

#[async_trait]
impl WidgetGroupMembershipSource for EmptyWidgetGroupMembershipSource {
    async fn list_memberships(
        &self,
        _stock_ids: &[String],
        _axis_key: &str,
    ) -> Result<Vec<core_application::stock_group::StockGroupMembership>, WidgetQueryError> {
        Ok(Vec::new())
    }
}

#[async_trait]
impl WidgetQuerySource for FakeWidgetQuerySource {
    async fn resolve_as_of(
        &self,
        requested: Option<NaiveDate>,
    ) -> Result<Option<NaiveDate>, WidgetQueryError> {
        if self
            .unavailable_date
            .is_some_and(|date| requested == Some(date))
        {
            return Ok(None);
        }
        Ok(requested.or_else(|| NaiveDate::from_ymd_opt(2099, 2, 3)))
    }

    async fn query(
        &self,
        _as_of: NaiveDate,
        definition: &WidgetQuery,
    ) -> Result<Vec<WidgetQueryRow>, WidgetQueryError> {
        if self.incompatible_metric == Some(definition.metric) {
            return Err(WidgetQueryError::IncompatibleCurrencies);
        }
        Ok(vec![WidgetQueryRow {
            key: WidgetRowKey::Market,
            name: "Market Sample".to_owned(),
            value: Decimal::new(125, 2),
        }])
    }
}

fn test_server(db: gateway_postgres::DatabaseHandle) -> super::tests_common::MgmtTestServer {
    test_server_with_source(
        db,
        FakeWidgetQuerySource {
            unavailable_date: None,
            incompatible_metric: None,
        },
    )
}

fn test_server_with_source(
    db: gateway_postgres::DatabaseHandle,
    source: FakeWidgetQuerySource,
) -> super::tests_common::MgmtTestServer {
    build_server(db, Arc::new(FakeAgentTaskClient::new())).with_widgets(WidgetUseCases::new(
        Arc::new(source),
        Arc::new(EmptyWidgetGroupMembershipSource) as SharedWidgetGroupMembershipSource,
    ))
}

#[backend_test_macros::database_test]
async fn preview_returns_calculated_widget_data_and_an_encoded_url(
    db: gateway_postgres::DatabaseHandle,
) {
    let server = test_server(db);
    let result = server.preview_dashboard(DASHBOARD_YAML).await.unwrap();

    assert_eq!(
        result.0,
        json!({
            "errors": [],
            "preview_url": concat!(
                "/dashboards/preview#",
                "dGl0bGU6IHNhbXBsZS1kYXNoYm9hcmQKd2lkZ2V0czoKICAtIHRpdGxlOiBzYW1wbGUtd2lkZ2V0CiAgICB3aWR0aDogZnVsbAogICAgcXVlcmllczoKICAgICAgbW92ZW1lbnQ6CiAgICAgICAgbWV0cmljOiBjaGFuZ2UKICAgICAgICBwZXJpb2Q6IDFkCiAgICAgICAgYnk6IG5vbmUKICAgICAgICBhZ2c6IG1lYW4KICAgIHZpejoKICAgICAgdHlwZTogdG9wbGlzdAogICAgICB2YWx1ZTogbW92ZW1lbnQKICAgICAgb3JkZXI6IGRlc2MKICAgICAgbGltaXQ6IDEK",
            ),
            "widgets": [{
                "title": "sample-widget",
                "width": "full",
                "as_of": "2099-02-03",
                "rows": [{
                    "key": "market",
                    "name": "Market Sample",
                    "values": { "movement": 1.25 },
                }],
            }],
        }),
    );
}

#[backend_test_macros::database_test]
async fn preview_returns_validation_paths_without_widget_results(
    db: gateway_postgres::DatabaseHandle,
) {
    let server = test_server(db);
    let yaml = indoc! {"
        title: sample-dashboard
        widgets:
          - title: sample-widget
            width: half
            queries: {}
            viz:
              type: toplist
              value: missing
              order: desc
              limit: 1
    "};
    let result = server.preview_dashboard(yaml).await.unwrap();

    assert_eq!(
        result.0,
        json!({
            "errors": [
                {
                    "path": "widgets[0].queries",
                    "message": "at least one query is required",
                },
                {
                    "path": "widgets[0].viz.value",
                    "message": "unknown query or formula name: `missing`",
                },
            ],
            "preview_url": null,
            "widgets": [],
        }),
    );
}

#[backend_test_macros::database_test]
async fn preview_reports_malformed_yaml(db: gateway_postgres::DatabaseHandle) {
    let server = test_server(db);
    let result = server
        .preview_dashboard("title: 'unterminated")
        .await
        .unwrap();

    assert_eq!(
        result.0,
        json!({
            "errors": [{
                "path": "$",
                "message": "found unexpected end of stream at line 1 column 21, while scanning a quoted scalar at line 1 column 8",
            }],
            "preview_url": null,
            "widgets": [],
        }),
    );
}

#[backend_test_macros::database_test]
async fn preview_continues_after_a_widget_has_no_data(db: gateway_postgres::DatabaseHandle) {
    let unavailable_date = NaiveDate::from_ymd_opt(2099, 2, 2);
    let server = test_server_with_source(
        db,
        FakeWidgetQuerySource {
            unavailable_date,
            incompatible_metric: None,
        },
    );
    let yaml = indoc! {"
        title: sample-dashboard
        widgets:
          - title: unavailable-widget
            width: half
            queries:
              movement:
                metric: change
                period: 1d
                by: none
                agg: mean
            time:
              as_of: 2099-02-02
            viz:
              type: toplist
              value: movement
              order: desc
              limit: 1
          - title: available-widget
            width: full
            queries:
              movement:
                metric: change
                period: 1d
                by: none
                agg: mean
            viz:
              type: toplist
              value: movement
              order: desc
              limit: 1
    "};
    let result = server.preview_dashboard(yaml).await.unwrap();
    let mut normalized_result = result.0;
    if normalized_result["preview_url"].as_str().is_some() {
        normalized_result["preview_url"] = json!("<preview_url>");
    }

    assert_eq!(
        normalized_result,
        json!({
            "errors": [{
                "path": "widgets[0].time.as_of",
                "message": "no widget data is available on or before the requested date",
            }],
            "preview_url": "<preview_url>",
            "widgets": [{
                "title": "available-widget",
                "width": "full",
                "as_of": "2099-02-03",
                "rows": [{
                    "key": "market",
                    "name": "Market Sample",
                    "values": { "movement": 1.25 },
                }],
            }],
        }),
    );
}

#[backend_test_macros::database_test]
async fn preview_continues_after_a_widget_has_incompatible_currencies(
    db: gateway_postgres::DatabaseHandle,
) {
    let server = test_server_with_source(
        db,
        FakeWidgetQuerySource {
            unavailable_date: None,
            incompatible_metric: Some(WidgetMetric::Close),
        },
    );
    let yaml = indoc! {"
        title: sample-dashboard
        widgets:
          - title: incompatible-widget
            width: half
            queries:
              amount:
                metric: close
                by: none
                agg: sum
            viz:
              type: table
              columns:
                - amount
          - title: available-widget
            width: full
            queries:
              movement:
                metric: change
                period: 1d
                by: none
                agg: mean
            viz:
              type: toplist
              value: movement
              order: desc
              limit: 1
    "};
    let result = server.preview_dashboard(yaml).await.unwrap();
    let mut normalized_result = result.0;
    if normalized_result["preview_url"].as_str().is_some() {
        normalized_result["preview_url"] = json!("<preview_url>");
    }

    assert_eq!(
        normalized_result,
        json!({
            "errors": [{
                "path": "widgets[0].queries",
                "message": "amount aggregation across multiple stocks requires confirmed currencies",
            }],
            "preview_url": "<preview_url>",
            "widgets": [{
                "title": "available-widget",
                "width": "full",
                "as_of": "2099-02-03",
                "rows": [{
                    "key": "market",
                    "name": "Market Sample",
                    "values": { "movement": 1.25 },
                }],
            }],
        }),
    );
}
