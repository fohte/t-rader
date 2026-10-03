#[cfg(test)]
mod tests {
    use rstest::rstest;
    use serde_json::json;
    use uuid::Uuid;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::super::tests_common::mock_db_with_strategy;
    use gateway_litellm::LiteLlmClient;

    use super::super::dto::QueryMediaParams;

    fn params(media_url: &str, prompt: &str) -> QueryMediaParams {
        QueryMediaParams {
            media_url: media_url.into(),
            prompt: prompt.into(),
        }
    }

    #[tokio::test]
    async fn query_media_returns_model_text() {
        let litellm = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "choices": [{"message": {"content": "銘柄Aについて言及"}}],
            })))
            .mount(&litellm)
            .await;

        let client = LiteLlmClient::new(&litellm.uri(), None).expect("build client");
        let strategy_id = Uuid::new_v4();
        let server = super::super::tests_common::build_server(mock_db_with_strategy(strategy_id))
            .with_litellm_client(Some(std::sync::Arc::new(client)));

        let out = server
            .query_media(
                strategy_id,
                "example-model-media".to_string(),
                params("https://www.youtube.com/watch?v=abc", "銘柄を列挙して"),
            )
            .await
            .expect("query_media");
        let requests = litellm
            .received_requests()
            .await
            .expect("recorded requests");
        let body: serde_json::Value = requests[0].body_json().expect("parse request body");
        assert_eq!(
            (out.as_json().clone(), body),
            (
                json!({"text": "銘柄Aについて言及"}),
                json!({
                    "model": "example-model-media",
                    "messages": [{
                        "role": "user",
                        "content": [
                            {"type": "text", "text": "銘柄を列挙して"},
                            {"type": "file", "file": {"file_id": "https://www.youtube.com/watch?v=abc"}},
                        ],
                    }],
                }),
            ),
        );
    }

    #[tokio::test]
    async fn query_media_requires_litellm_client() {
        let strategy_id = Uuid::new_v4();
        let server = super::super::tests_common::build_server(mock_db_with_strategy(strategy_id));
        let err = server
            .query_media(
                strategy_id,
                "example-model-media".to_string(),
                params("https://example.com/v.mp4", "説明して"),
            )
            .await
            .expect_err("expected internal error");
        assert_eq!(
            err,
            rmcp::ErrorData::internal_error("litellm client is not configured", None),
        );
    }

    #[rstest]
    #[case::empty_media_url("", "prompt", "media_url must not be empty")]
    #[case::empty_prompt("https://example.com/v.mp4", "", "prompt must not be empty")]
    #[tokio::test]
    async fn query_media_rejects_invalid_params(
        #[case] media_url: &str,
        #[case] prompt: &str,
        #[case] expected_msg: &str,
    ) {
        let strategy_id = Uuid::new_v4();
        let server = super::super::tests_common::build_server(mock_db_with_strategy(strategy_id));
        let err = server
            .query_media(
                strategy_id,
                "example-model-media".to_string(),
                params(media_url, prompt),
            )
            .await
            .expect_err("expected invalid params");
        assert_eq!(
            err,
            rmcp::ErrorData::invalid_params(expected_msg.to_string(), None),
        );
    }
}
