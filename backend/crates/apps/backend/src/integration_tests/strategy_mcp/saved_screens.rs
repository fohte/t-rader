use core_application::saved_screen::SavedScreenData;
use core_application::screener::{
    ScreenerFilters, ScreenerRange, ScreenerSort, ScreenerSortDirection, ScreenerSortField,
};
use rust_decimal::Decimal;
use serde_json::{Value, json};
use uuid::Uuid;

use super::tests_common::build_server;

#[backend_test_macros::database_test]
async fn list_saved_screens_returns_names_filters_and_sort(db: gateway_postgres::DatabaseHandle) {
    let use_cases = crate::testing::build_use_cases_without_agent_task_client_for_test(db.clone());
    let _saved = use_cases
        .saved_screens()
        .create(SavedScreenData {
            name: "condition-alpha".to_owned(),
            filters: ScreenerFilters {
                markets: vec!["market-a".to_owned()],
                sectors: vec!["sector-a".to_owned()],
                per: Some(ScreenerRange {
                    min: Some(Decimal::new(825, 2)),
                    max: Some(Decimal::new(145, 1)),
                }),
                ..Default::default()
            },
            sort: ScreenerSort {
                field: ScreenerSortField::Per,
                direction: ScreenerSortDirection::Asc,
            },
        })
        .await
        .expect("create saved screen");
    let server = build_server(db);
    let result = server
        .invoke::<_, Value>("list_saved_screens", Uuid::new_v4(), json!({}), None, None)
        .await
        .expect("list saved screens")
        .as_json()
        .clone();
    let mut normalized = result;
    normalized["screens"][0]["id"] = json!("<id>");

    assert_eq!(
        normalized,
        json!({
            "screens": [{
                "id": "<id>",
                "name": "condition-alpha",
                "filters": {
                    "markets": ["market-a"],
                    "sectors": ["sector-a"],
                    "per": { "min": 8.25, "max": 14.5 },
                    "pbr": null,
                    "roe": null,
                    "dividend_yield": null,
                    "market_cap": null,
                },
                "sort": { "field": "per", "direction": "asc" },
            }],
        }),
    );
}
