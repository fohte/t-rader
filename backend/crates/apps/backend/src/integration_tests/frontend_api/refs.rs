#[cfg(test)]
mod tests {
    use super::super::assert_response_eq;
    use serde_json::{Value, json};

    use crate::testing::{
        create_test_server_with_db, insert_test_group, insert_test_group_membership,
        insert_test_stock,
    };

    fn normalize_stock_timestamps(stocks: &mut Value) {
        for stock in stocks.as_array_mut().expect("stock list") {
            stock["created_at"] = json!("normalized timestamp");
            stock["updated_at"] = json!("normalized timestamp");
        }
    }

    #[backend_test_macros::database_test]
    async fn list_stocks_filters_by_query(db: gateway_postgres::DatabaseHandle) {
        let (db, server) = create_test_server_with_db(db).await;
        insert_test_stock(&db, "MOCK_001", "Mock Alpha").await;
        insert_test_stock(&db, "MOCK_002", "Mock Beta").await;
        insert_test_group_membership(
            &db,
            "MOCK_001",
            "synthetic-derived-axis",
            "synthetic-industry",
            "Sample Industry",
            Some("tse_sector33"),
        )
        .await;

        let response = server.get("/api/refs/stocks?q=Alpha").await;
        let mut actual = response.json::<Value>();
        normalize_stock_timestamps(&mut actual);

        assert_eq!(
            (response.status_code(), actual),
            (
                axum::http::StatusCode::OK,
                json!([{
                    "id": "MOCK_001",
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
