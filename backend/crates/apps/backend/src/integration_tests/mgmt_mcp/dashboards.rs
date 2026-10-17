use std::sync::Arc;

use async_trait::async_trait;
use chrono::NaiveDate;
use core_application::widget::{
    WidgetQuery, WidgetQueryError, WidgetQueryRow, WidgetQuerySource, WidgetRowKey, WidgetUseCases,
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

struct FakeWidgetQuerySource;

#[async_trait]
impl WidgetQuerySource for FakeWidgetQuerySource {
    async fn resolve_as_of(
        &self,
        requested: Option<NaiveDate>,
    ) -> Result<Option<NaiveDate>, WidgetQueryError> {
        Ok(requested.or_else(|| NaiveDate::from_ymd_opt(2099, 2, 3)))
    }

    async fn query(
        &self,
        _as_of: NaiveDate,
        _definition: &WidgetQuery,
    ) -> Result<Vec<WidgetQueryRow>, WidgetQueryError> {
        Ok(vec![WidgetQueryRow {
            key: WidgetRowKey::Market,
            name: "Market Sample".to_owned(),
            value: Decimal::new(125, 2),
        }])
    }
}

fn test_server(db: gateway_postgres::DatabaseHandle) -> super::tests_common::MgmtTestServer {
    build_server(db, Arc::new(FakeAgentTaskClient::new()))
        .with_widgets(WidgetUseCases::new(Arc::new(FakeWidgetQuerySource)))
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
