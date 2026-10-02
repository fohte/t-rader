#[cfg(test)]
mod tests {
    use crate::testing::create_test_server;

    #[backend_test_macros::database_test]
    async fn get_config_returns_null_when_env_unset(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        let response = server.get("/api/config").await;
        response.assert_status_ok();
        assert_eq!(
            response.json::<serde_json::Value>(),
            serde_json::json!({ "trace_url_template": null }),
        );
    }
}
