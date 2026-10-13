#[cfg(test)]
mod tests {
    use super::super::assert_response_eq;
    use chrono::NaiveDate;
    use core_application::indicator_observation::{
        IndicatorObservationMetadata, IndicatorObservationRepository,
    };
    use core_domain::IndicatorObservation;
    use rust_decimal::Decimal;
    use serde_json::{Value, json};

    use crate::testing::{
        create_test_server_with_db, insert_test_group, insert_test_group_membership,
        insert_test_stock,
    };
    use gateway_postgres::PostgresIndicatorObservationRepository;
    use rstest::{fixture, rstest};

    const DEMO_INDICATOR_ID: &str = "DEMO_INDICATOR";

    fn observation(day: u32, value: i64) -> IndicatorObservation {
        IndicatorObservation {
            date: NaiveDate::from_ymd_opt(2040, 1, day).expect("valid date"),
            value: Decimal::from(value),
        }
    }

    #[fixture]
    async fn indicator_observations_context()
    -> (gateway_postgres::DatabaseHandle, axum_test::TestServer) {
        let db = gateway_postgres::test_support::create_test_transaction().await;
        let (db, server) = create_test_server_with_db(db).await;
        let repository = PostgresIndicatorObservationRepository::new(db.clone());
        repository
            .ensure_indicator(IndicatorObservationMetadata {
                indicator_id: DEMO_INDICATOR_ID.to_string(),
                name: "Demo Indicator".to_string(),
                kind: "index".to_string(),
            })
            .await
            .expect("insert demo indicator");
        repository
            .upsert_observations(
                DEMO_INDICATOR_ID,
                vec![observation(3, 30), observation(1, 10), observation(2, 20)],
            )
            .await
            .expect("insert demo observations");
        (db, server)
    }

    fn normalize_stock_timestamps(stocks: &mut Value) {
        for stock in stocks.as_array_mut().expect("stock list") {
            stock["created_at"] = json!("normalized timestamp");
            stock["updated_at"] = json!("normalized timestamp");
        }
    }

    #[fixture]
    async fn stock_search_context() -> (gateway_postgres::DatabaseHandle, axum_test::TestServer) {
        let db = gateway_postgres::test_support::create_test_transaction().await;
        let (db, server) = create_test_server_with_db(db).await;
        insert_test_stock(&db, "MOCK-001", "Mock Alpha").await;
        insert_test_stock(&db, "MOCK-002", "Mock Beta").await;
        insert_test_group_membership(
            &db,
            "MOCK-001",
            "synthetic-derived-axis",
            "synthetic-industry",
            "Sample Industry",
            Some("tse_sector33"),
        )
        .await;
        (db, server)
    }

    #[rstest]
    #[case::uppercase_stock_id("MOCK-001")]
    #[case::lowercase_stock_id("mock-001")]
    #[case::uppercase_name("Alpha")]
    #[case::lowercase_name("alpha")]
    #[tokio::test]
    async fn list_stocks_filters_by_query(
        #[future] stock_search_context: (gateway_postgres::DatabaseHandle, axum_test::TestServer),
        #[case] query: &str,
    ) {
        let (_, server) = stock_search_context.await;

        let response = server.get(&format!("/api/refs/stocks?q={query}")).await;
        let mut actual = response.json::<Value>();
        normalize_stock_timestamps(&mut actual);

        assert_eq!(
            (response.status_code(), actual),
            (
                axum::http::StatusCode::OK,
                json!([{
                    "id": "MOCK-001",
                    "name": "Mock Alpha",
                    "market": null,
                    "sector_id": "synthetic-industry",
                    "created_at": "normalized timestamp",
                    "updated_at": "normalized timestamp",
                    "product_category": null,
                }]),
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn get_stock_returns_404_for_unknown_id(db: gateway_postgres::DatabaseHandle) {
        let (_db, server) = create_test_server_with_db(db).await;

        let response = server.get("/api/refs/stocks/UNKNOWN").await;

        assert_response_eq(
            &response,
            axum::http::StatusCode::NOT_FOUND,
            Some(json!({ "error": "stock UNKNOWN not found" })),
        );
    }

    #[rstest]
    #[case::no_bounds(
        "",
        axum::http::StatusCode::OK,
        json!([
            { "date": "2040-01-01", "value": 10 },
            { "date": "2040-01-02", "value": 20 },
            { "date": "2040-01-03", "value": 30 },
        ]),
    )]
    #[case::from_only(
        "?from=2040-01-02",
        axum::http::StatusCode::OK,
        json!([
            { "date": "2040-01-02", "value": 20 },
            { "date": "2040-01-03", "value": 30 },
        ]),
    )]
    #[case::to_only(
        "?to=2040-01-02",
        axum::http::StatusCode::OK,
        json!([
            { "date": "2040-01-01", "value": 10 },
            { "date": "2040-01-02", "value": 20 },
        ]),
    )]
    #[case::both_bounds(
        "?from=2040-01-02&to=2040-01-03",
        axum::http::StatusCode::OK,
        json!([
            { "date": "2040-01-02", "value": 20 },
            { "date": "2040-01-03", "value": 30 },
        ]),
    )]
    #[case::reversed_range(
        "?from=2040-01-03&to=2040-01-02",
        axum::http::StatusCode::BAD_REQUEST,
        json!({ "error": "from must be on or before to" }),
    )]
    #[tokio::test]
    async fn get_indicator_observations_handles_optional_bounds_and_invalid_ranges(
        #[future] indicator_observations_context: (
            gateway_postgres::DatabaseHandle,
            axum_test::TestServer,
        ),
        #[case] query: &str,
        #[case] expected_status: axum::http::StatusCode,
        #[case] expected: Value,
    ) {
        let (_, server) = indicator_observations_context.await;

        let response = server
            .get(&format!(
                "/api/refs/indicators/DEMO_INDICATOR/observations{query}"
            ))
            .await;

        assert_response_eq(&response, expected_status, Some(expected));
    }

    #[backend_test_macros::database_test]
    async fn get_indicator_observations_returns_404_for_an_unknown_indicator(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let (_db, server) = create_test_server_with_db(db).await;

        let response = server
            .get("/api/refs/indicators/DEMO_UNKNOWN/observations")
            .await;

        assert_response_eq(
            &response,
            axum::http::StatusCode::NOT_FOUND,
            Some(json!({ "error": "indicator DEMO_UNKNOWN not found" })),
        );
    }

    #[backend_test_macros::database_test]
    async fn resolve_group_link_returns_its_name(db: gateway_postgres::DatabaseHandle) {
        let (db, server) = create_test_server_with_db(db).await;
        insert_test_group(&db, "demo-axis", "demo-group", "Sample Group").await;

        let response = server
            .get("/api/refs/resolve?link=group%3Ademo-axis%2Fdemo-group")
            .await;

        assert_response_eq(
            &response,
            axum::http::StatusCode::OK,
            Some(json!([{
                "kind": "group",
                "id": "demo-axis/demo-group",
                "name": "Sample Group",
            }])),
        );
    }

    #[backend_test_macros::database_test]
    async fn resolve_group_link_rejects_a_missing_axis_key(db: gateway_postgres::DatabaseHandle) {
        let (_db, server) = create_test_server_with_db(db).await;

        let response = server
            .get("/api/refs/resolve?link=group%3Ademo-group")
            .await;

        assert_response_eq(
            &response,
            axum::http::StatusCode::BAD_REQUEST,
            Some(json!({ "error": "invalid group ref_id: demo-group" })),
        );
    }
}
