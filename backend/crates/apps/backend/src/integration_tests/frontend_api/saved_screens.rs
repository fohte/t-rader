#[cfg(test)]
mod tests {
    use axum::http::StatusCode;
    use axum_test::TestServer;
    use serde_json::{Value, json};
    use uuid::Uuid;

    use super::super::assert_response_eq;
    use crate::testing::create_test_server;

    fn request(name: &str, markets: &[&str], min_per: Option<i64>, max_per: Option<i64>) -> Value {
        let per = match (min_per, max_per) {
            (None, None) => Value::Null,
            (min, max) => json!({ "min": min, "max": max }),
        };
        json!({
            "name": name,
            "filters": {
                "markets": markets,
                "sectors": [],
                "per": per,
                "pbr": null,
                "roe": null,
                "dividend_yield": null,
                "market_cap": null,
            },
            "sort": { "field": "market_cap", "direction": "desc" },
        })
    }

    fn expected(name: &str, markets: &[&str], min_per: Option<i64>, max_per: Option<i64>) -> Value {
        let mut value = request(name, markets, min_per, max_per);
        value
            .as_object_mut()
            .expect("request object")
            .remove("name");
        json!({
            "id": "<id>",
            "name": name,
            "filters": value["filters"],
            "sort": value["sort"],
            "created_at": "<created_at>",
            "updated_at": "<updated_at>",
        })
    }

    fn normalize_row(mut row: Value) -> Value {
        for key in ["id", "created_at", "updated_at"] {
            row[key] = json!(format!("<{key}>"));
        }
        for field in ["per", "pbr", "roe", "dividend_yield", "market_cap"] {
            for bound in ["min", "max"] {
                if let Some(Value::Number(number)) = row["filters"][field].get(bound) {
                    let decimal = number
                        .to_string()
                        .parse::<rust_decimal::Decimal>()
                        .expect("saved screen range is numeric");
                    row["filters"][field][bound] =
                        serde_json::from_str(&decimal.normalize().to_string())
                            .expect("normalized saved screen range");
                }
            }
        }
        row
    }

    fn normalize_rows(value: Value) -> Value {
        match value {
            Value::Array(rows) => Value::Array(rows.into_iter().map(normalize_row).collect()),
            row => normalize_row(row),
        }
    }

    async fn create_screen(server: &TestServer, body: Value) -> Uuid {
        let response = server.post("/api/saved-screens").json(&body).await;
        response
            .json::<Value>()
            .get("id")
            .and_then(Value::as_str)
            .expect("saved screen id")
            .parse()
            .expect("valid saved screen UUID")
    }

    #[backend_test_macros::database_test]
    async fn create_returns_saved_filters_and_sort(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        let response = server
            .post("/api/saved-screens")
            .json(&request(
                "  condition-alpha  ",
                &["market-a"],
                Some(5),
                Some(12),
            ))
            .await;

        assert_eq!(
            (response.status_code(), normalize_rows(response.json())),
            (
                StatusCode::CREATED,
                expected("condition-alpha", &["market-a"], Some(5), Some(12)),
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn list_returns_saved_screens_in_name_order(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        create_screen(
            &server,
            request("condition-zeta", &["market-b"], None, Some(20)),
        )
        .await;
        create_screen(
            &server,
            request("condition-alpha", &["market-a"], Some(3), None),
        )
        .await;

        let response = server.get("/api/saved-screens").await;
        assert_eq!(
            (response.status_code(), normalize_rows(response.json())),
            (
                StatusCode::OK,
                json!([
                    expected("condition-alpha", &["market-a"], Some(3), None),
                    expected("condition-zeta", &["market-b"], None, Some(20)),
                ]),
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn update_replaces_name_filters_and_sort(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        let id = create_screen(
            &server,
            request("condition-before", &["market-a"], None, None),
        )
        .await;
        let updated = server
            .put(&format!("/api/saved-screens/{id}"))
            .json(&json!({
                "name": "condition-after",
                "filters": {
                    "markets": ["market-b"],
                    "sectors": ["sector-a"],
                    "per": { "min": 8, "max": 15 },
                    "pbr": null,
                    "roe": null,
                    "dividend_yield": null,
                    "market_cap": null,
                },
                "sort": { "field": "per", "direction": "asc" },
            }))
            .await;
        let listed = server.get("/api/saved-screens").await;
        let expected_row = json!({
            "id": "<id>",
            "name": "condition-after",
            "filters": {
                "markets": ["market-b"],
                "sectors": ["sector-a"],
                "per": { "min": 8, "max": 15 },
                "pbr": null,
                "roe": null,
                "dividend_yield": null,
                "market_cap": null,
            },
            "sort": { "field": "per", "direction": "asc" },
            "created_at": "<created_at>",
            "updated_at": "<updated_at>",
        });

        assert_eq!(
            (
                updated.status_code(),
                normalize_rows(updated.json()),
                listed.status_code(),
                normalize_rows(listed.json()),
            ),
            (
                StatusCode::OK,
                expected_row.clone(),
                StatusCode::OK,
                json!([expected_row]),
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn delete_removes_saved_screen(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        let id = create_screen(
            &server,
            request("condition-alpha", &["market-a"], None, None),
        )
        .await;
        let deleted = server.delete(&format!("/api/saved-screens/{id}")).await;
        let listed = server.get("/api/saved-screens").await;

        assert_eq!(
            (
                deleted.status_code(),
                listed.status_code(),
                listed.json::<Value>()
            ),
            (StatusCode::NO_CONTENT, StatusCode::OK, json!([])),
        );
    }

    #[backend_test_macros::database_test]
    async fn reversed_range_is_rejected(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        let response = server
            .post("/api/saved-screens")
            .json(&request(
                "condition-invalid",
                &["market-a"],
                Some(20),
                Some(10),
            ))
            .await;

        assert_response_eq(
            &response,
            StatusCode::BAD_REQUEST,
            Some(json!({ "error": "per minimum must be less than or equal to maximum" })),
        );
    }

    #[backend_test_macros::database_test]
    async fn duplicate_name_returns_conflict(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        create_screen(
            &server,
            request("condition-alpha", &["market-a"], None, None),
        )
        .await;
        let response = server
            .post("/api/saved-screens")
            .json(&request("condition-alpha", &["market-b"], None, None))
            .await;

        assert_response_eq(
            &response,
            StatusCode::CONFLICT,
            Some(json!({ "error": "saved screen 'condition-alpha' already exists" })),
        );
    }
}
