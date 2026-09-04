//! `query_media` tool の inner method 実装。
//!
//! 動画/音声 URL (YouTube の公開動画 URL を主対象) を Gemini に渡し、prompt の指示に
//! 沿ったテキスト応答を返す。discover フェーズがテキストにしか無い材料にアクセス
//! できるようにするための tool。

use rmcp::ErrorData as McpError;
use uuid::Uuid;

use crate::services::litellm_client::{ChatMessage, ContentPart, FilePart};

use super::dto::{QueryMediaParams, QueryMediaResult};
use super::{StrategyServer, internal_error, invalid_params, litellm_error_to_mcp};

/// Vertex AI 経由で Google AI Pro 付帯クレジットでの動作を確認済みのモデル。
/// `gemini-3.1-pro` はアクセス権が無く 404/429 になるため使わない。
const GEMINI_MEDIA_MODEL: &str = "gemini-3.6-flash";

impl StrategyServer {
    pub(crate) async fn query_media_inner(
        &self,
        session_strategy_id: Uuid,
        params: QueryMediaParams,
    ) -> Result<QueryMediaResult, McpError> {
        let media_url = params.media_url.trim().to_string();
        if media_url.is_empty() {
            return Err(invalid_params("media_url must not be empty"));
        }
        let prompt = params.prompt.trim().to_string();
        if prompt.is_empty() {
            return Err(invalid_params("prompt must not be empty"));
        }

        let client = self
            .litellm_client
            .as_ref()
            .ok_or_else(|| internal_error("litellm client is not configured"))?;

        let messages = vec![ChatMessage {
            role: "user",
            content: vec![
                ContentPart::Text { text: prompt },
                ContentPart::File {
                    file: FilePart {
                        file_id: media_url.clone(),
                    },
                },
            ],
        }];

        tracing::info!(
            strategy_id = %session_strategy_id,
            model = GEMINI_MEDIA_MODEL,
            media_url,
            "query_media: dispatching chat completion request"
        );

        let text = client
            .chat_completion(GEMINI_MEDIA_MODEL, messages)
            .await
            .map_err(|e| {
                tracing::warn!(
                    strategy_id = %session_strategy_id,
                    model = GEMINI_MEDIA_MODEL,
                    media_url,
                    error = %e,
                    "query_media: chat completion request failed"
                );
                litellm_error_to_mcp(e)
            })?;

        Ok(QueryMediaResult { text })
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;
    use sea_orm::{DatabaseBackend, MockDatabase};
    use serde_json::json;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use crate::services::litellm_client::LiteLlmClient;

    use super::super::StrategyServer;
    use super::super::dto::{QueryMediaParams, QueryMediaResult};
    use super::*;

    fn mock_db() -> sea_orm::DatabaseConnection {
        MockDatabase::new(DatabaseBackend::Postgres).into_connection()
    }

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
        let server = StrategyServer::new(mock_db(), None).with_litellm_client(Some(client));

        let out = server
            .query_media_inner(
                Uuid::new_v4(),
                params("https://www.youtube.com/watch?v=abc", "銘柄を列挙して"),
            )
            .await
            .expect("query_media");
        assert_eq!(
            out,
            QueryMediaResult {
                text: "銘柄Aについて言及".into(),
            }
        );
    }

    #[tokio::test]
    async fn query_media_requires_litellm_client() {
        let server = StrategyServer::new(mock_db(), None);
        let err = server
            .query_media_inner(
                Uuid::new_v4(),
                params("https://example.com/v.mp4", "説明して"),
            )
            .await
            .expect_err("expected internal error");
        assert_eq!(
            (err.code, err.message.as_ref()),
            (
                rmcp::model::ErrorCode::INTERNAL_ERROR,
                "litellm client is not configured",
            ),
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
        let server = StrategyServer::new(mock_db(), None);
        let err = server
            .query_media_inner(Uuid::new_v4(), params(media_url, prompt))
            .await
            .expect_err("expected invalid params");
        assert_eq!(
            (err.code, err.message.as_ref()),
            (rmcp::model::ErrorCode::INVALID_PARAMS, expected_msg),
        );
    }
}
