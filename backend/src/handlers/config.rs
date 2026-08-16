use axum::Json;

use crate::models::ConfigResponse;

/// frontend が実行時に必要とする軽量な設定値をまとめて返す。DB を介さず env var を直接読む。
#[utoipa::path(
    get,
    path = "/api/config",
    tag = "config",
    responses((status = 200, body = ConfigResponse)),
)]
pub async fn get_config() -> Json<ConfigResponse> {
    Json(config_with(|key| std::env::var(key).ok()))
}

fn config_with<F>(get: F) -> ConfigResponse
where
    F: Fn(&str) -> Option<String>,
{
    ConfigResponse {
        trace_url_template: get("TRACE_URL_TEMPLATE").filter(|s| !s.is_empty()),
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;
    use sqlx::PgPool;

    use super::*;
    use crate::testing::create_test_server;

    fn env_get<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
        move |key: &str| {
            pairs
                .iter()
                .find(|(k, _)| *k == key)
                .map(|(_, v)| (*v).to_string())
        }
    }

    #[rstest]
    #[case::unset(&[], None)]
    #[case::empty(&[("TRACE_URL_TEMPLATE", "")], None)]
    #[case::set(
        &[("TRACE_URL_TEMPLATE", "https://example.com/{trace_id}")],
        Some("https://example.com/{trace_id}")
    )]
    fn config_with_resolves_env(#[case] env: &[(&str, &str)], #[case] expected: Option<&str>) {
        assert_eq!(
            config_with(env_get(env)),
            ConfigResponse {
                trace_url_template: expected.map(str::to_string)
            },
        );
    }

    #[sqlx::test(migrations = false)]
    async fn get_config_returns_null_when_env_unset(pool: PgPool) {
        let server = create_test_server(pool).await;
        let response = server.get("/api/config").await;
        response.assert_status_ok();
        assert_eq!(
            response.json::<serde_json::Value>(),
            serde_json::json!({ "trace_url_template": null }),
        );
    }
}
