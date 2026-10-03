#[cfg(test)]
mod tests {
    use super::super::assert_response_eq;
    use crate::testing::create_test_server;

    #[backend_test_macros::database_test]
    async fn get_config_returns_null_when_env_unset(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        let response = server.get("/api/config").await;
        assert_response_eq(
            &response,
            axum::http::StatusCode::OK,
            Some(serde_json::json!({ "trace_url_template": null })),
        );
    }
}
