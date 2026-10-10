//! LiteLLM Proxy を叩く薄いクライアント
//!
//! - `/model_group/info`: LiteLLM Proxy のモデルグループ情報を取得する
//! - `/v1/chat/completions`: chat completion を呼び出す
//!
//! LiteLLM の API キーを frontend に晒さないためだけの中継で、キャッシュはしない。

use std::time::Duration;

use async_trait::async_trait;
use core_application::llm_client::{
    ChatMessage, ContentPart, FilePart, LlmClient, LlmClientError, LlmModel,
};
use serde::{Deserialize, Serialize};

const REQUEST_TIMEOUT: Duration = Duration::from_secs(8);
/// chat completion はマルチモーダル入力の解析を伴い得るため、admin API より長い上限を使う。
const CHAT_COMPLETION_TIMEOUT: Duration = Duration::from_secs(120);

#[derive(Debug, Deserialize)]
struct ModelGroupInfoResponse {
    #[serde(default)]
    data: Vec<ModelGroupInfo>,
}

#[derive(Debug, Deserialize)]
struct ModelGroupInfo {
    model_group: String,
    #[serde(default)]
    providers: Vec<String>,
    #[serde(default)]
    max_input_tokens: Option<f64>,
    #[serde(default)]
    max_output_tokens: Option<f64>,
    #[serde(default)]
    supports_reasoning: bool,
}

impl From<ModelGroupInfo> for LlmModel {
    fn from(m: ModelGroupInfo) -> Self {
        Self {
            id: m.model_group,
            providers: m.providers,
            max_input_tokens: m.max_input_tokens,
            max_output_tokens: m.max_output_tokens,
            supports_reasoning: m.supports_reasoning,
        }
    }
}

#[derive(Debug, Serialize)]
struct WireChatMessage {
    role: &'static str,
    content: Vec<WireContentPart>,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum WireContentPart {
    Text { text: String },
    File { file: WireFilePart },
}

#[derive(Debug, Serialize)]
struct WireFilePart {
    file_id: String,
    #[serde(rename = "format", skip_serializing_if = "Option::is_none")]
    mime_type: Option<String>,
}

impl From<ChatMessage> for WireChatMessage {
    fn from(message: ChatMessage) -> Self {
        Self {
            role: message.role,
            content: message.content.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<ContentPart> for WireContentPart {
    fn from(part: ContentPart) -> Self {
        match part {
            ContentPart::Text { text } => Self::Text { text },
            ContentPart::File { file } => Self::File {
                file: WireFilePart::from(file),
            },
        }
    }
}

impl From<FilePart> for WireFilePart {
    fn from(file: FilePart) -> Self {
        Self {
            file_id: file.file_id,
            mime_type: file.mime_type,
        }
    }
}

#[derive(Debug, Serialize)]
struct ChatCompletionRequest<'a> {
    model: &'a str,
    messages: &'a [WireChatMessage],
}

#[derive(Debug, Deserialize)]
struct ChatCompletionResponse {
    #[serde(default)]
    choices: Vec<ChatCompletionChoice>,
}

#[derive(Debug, Deserialize)]
struct ChatCompletionChoice {
    message: ChatCompletionResponseMessage,
}

#[derive(Debug, Deserialize)]
struct ChatCompletionResponseMessage {
    #[serde(default)]
    content: Option<String>,
}

/// LiteLLM Proxy client。`LLM_BASE_URL` 未設定時は `from_env` が `None` を返す。
#[derive(Clone)]
pub struct LiteLlmClient {
    http: reqwest::Client,
    /// `/v1` サフィックスを除いた管理系 API のベース URL
    base_url: String,
}

impl LiteLlmClient {
    pub fn from_env() -> Option<Self> {
        Self::from_env_with(|key| std::env::var(key).ok())
    }

    fn from_env_with<F>(get: F) -> Option<Self>
    where
        F: Fn(&str) -> Option<String>,
    {
        let Some(base_url) = get("LLM_BASE_URL").filter(|s| !s.is_empty()) else {
            tracing::warn!("LLM_BASE_URL が未設定のため、litellm client を無効化します");
            return None;
        };
        let api_key = get("LLM_API_KEY").filter(|s| !s.is_empty());
        match Self::new(&base_url, api_key.as_deref()) {
            Ok(client) => Some(client),
            Err(e) => {
                tracing::warn!(error = %e, "failed to initialize litellm client");
                None
            }
        }
    }

    pub fn new(base_url: &str, api_key: Option<&str>) -> Result<Self, LlmClientError> {
        let mut headers = reqwest::header::HeaderMap::new();
        if let Some(key) = api_key {
            let value = reqwest::header::HeaderValue::from_str(&format!("Bearer {key}"))
                .map_err(|e| LlmClientError::Init(format!("invalid api key: {e}")))?;
            headers.insert(reqwest::header::AUTHORIZATION, value);
        }
        let http = reqwest::Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .default_headers(headers)
            .build()
            .map_err(|e| LlmClientError::Init(format!("failed to build http client: {e}")))?;

        // OpenAI 互換の `LLM_BASE_URL` (例: `http://litellm/v1`) と、LiteLLM 管理系 API の
        // ベース URL は別物。`/model_group/info` は `/v1` の下ではなくルート直下にある。
        let trimmed = base_url.trim_end_matches('/');
        let base_url = trimmed.strip_suffix("/v1").unwrap_or(trimmed).to_string();

        Ok(Self { http, base_url })
    }
}

#[async_trait]
impl LlmClient for LiteLlmClient {
    async fn list_models(&self) -> Result<Vec<LlmModel>, LlmClientError> {
        let url = format!("{}/model_group/info", self.base_url);
        let response = self
            .http
            .get(url)
            .send()
            .await
            .map_err(|e| LlmClientError::Network(e.to_string()))?;

        let status = response.status();
        if !status.is_success() {
            let message = response.text().await.unwrap_or_default();
            return Err(LlmClientError::Api {
                status: status.as_u16(),
                message,
            });
        }

        let body: ModelGroupInfoResponse = response
            .json()
            .await
            .map_err(|e| LlmClientError::Parse(e.to_string()))?;
        Ok(body.data.into_iter().map(LlmModel::from).collect())
    }

    /// OpenAI 互換の `/v1/chat/completions` を叩き、先頭 choice のメッセージ本文を返す。
    async fn chat_completion(
        &self,
        model: &str,
        messages: Vec<ChatMessage>,
    ) -> Result<String, LlmClientError> {
        let messages = messages.into_iter().map(Into::into).collect::<Vec<_>>();
        let url = format!("{}/v1/chat/completions", self.base_url);
        let response = self
            .http
            .post(url)
            .timeout(CHAT_COMPLETION_TIMEOUT)
            .json(&ChatCompletionRequest {
                model,
                messages: &messages,
            })
            .send()
            .await
            .map_err(|e| LlmClientError::Network(e.to_string()))?;

        let status = response.status();
        if !status.is_success() {
            let message = response.text().await.unwrap_or_default();
            return Err(LlmClientError::Api {
                status: status.as_u16(),
                message,
            });
        }

        let body: ChatCompletionResponse = response
            .json()
            .await
            .map_err(|e| LlmClientError::Parse(e.to_string()))?;

        body.choices
            .into_iter()
            .next()
            .and_then(|c| c.message.content)
            .ok_or_else(|| {
                LlmClientError::Parse("chat completion response has no message content".into())
            })
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;
    use serde_json::json;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;

    fn env_get<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
        move |key: &str| {
            pairs
                .iter()
                .find(|(k, _)| *k == key)
                .map(|(_, v)| (*v).to_string())
        }
    }

    #[rstest]
    #[case::missing_url(&[])]
    #[case::empty_url(&[("LLM_BASE_URL", "")])]
    fn from_env_returns_none_when_base_url_missing(#[case] env: &[(&str, &str)]) {
        assert!(LiteLlmClient::from_env_with(env_get(env)).is_none());
    }

    #[rstest]
    #[case::plain("http://litellm.local:4000", "http://litellm.local:4000")]
    #[case::v1_suffix("http://litellm.local:4000/v1", "http://litellm.local:4000")]
    #[case::v1_suffix_trailing_slash("http://litellm.local:4000/v1/", "http://litellm.local:4000")]
    fn new_strips_v1_suffix(#[case] input: &str, #[case] expected_base_url: &str) {
        let client = LiteLlmClient::new(input, None).expect("build client");
        assert_eq!(client.base_url, expected_base_url);
    }

    #[tokio::test]
    async fn list_models_parses_model_group_info_response() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/model_group/info"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "data": [
                    {
                        "model_group": "sample-model",
                        "providers": ["test-provider"],
                        "max_input_tokens": 200000.0,
                        "max_output_tokens": 8192.0,
                        "supports_reasoning": true,
                    },
                ],
            })))
            .mount(&server)
            .await;

        let client = LiteLlmClient::new(&server.uri(), None).expect("build client");
        let models = client.list_models().await.expect("list models ok");
        assert_eq!(
            models,
            vec![LlmModel {
                id: "sample-model".to_string(),
                providers: vec!["test-provider".to_string()],
                max_input_tokens: Some(200000.0),
                max_output_tokens: Some(8192.0),
                supports_reasoning: true,
            }],
        );
    }

    #[tokio::test]
    async fn list_models_maps_non_success_status_to_api_error() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/model_group/info"))
            .respond_with(ResponseTemplate::new(503).set_body_string("upstream unavailable"))
            .mount(&server)
            .await;

        let client = LiteLlmClient::new(&server.uri(), None).expect("build client");
        let err = client.list_models().await.expect_err("expected error");
        assert!(matches!(
            err,
            LlmClientError::Api { status: 503, message } if message == "upstream unavailable"
        ));
    }

    #[tokio::test]
    async fn chat_completion_returns_first_choice_message_content() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "choices": [{"message": {"content": "video summary"}}],
            })))
            .mount(&server)
            .await;

        let client = LiteLlmClient::new(&server.uri(), None).expect("build client");
        let text = client
            .chat_completion(
                "sample-chat-model",
                vec![ChatMessage {
                    role: "user",
                    content: vec![ContentPart::Text {
                        text: "describe this".into(),
                    }],
                }],
            )
            .await
            .expect("chat completion ok");
        assert_eq!(text, "video summary");
    }

    #[tokio::test]
    async fn chat_completion_serializes_file_content_part() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "choices": [{"message": {"content": "sample response"}}],
            })))
            .mount(&server)
            .await;

        let client = LiteLlmClient::new(&server.uri(), None).expect("build client");
        let _response = client
            .chat_completion(
                "sample-chat-model",
                vec![ChatMessage {
                    role: "user",
                    content: vec![
                        ContentPart::Text {
                            text: "summarize this sample".into(),
                        },
                        ContentPart::File {
                            file: FilePart {
                                file_id: "https://media.example.invalid/video".into(),
                                mime_type: Some("video/mp4".into()),
                            },
                        },
                    ],
                }],
            )
            .await
            .expect("chat completion ok");
        let request = server
            .received_requests()
            .await
            .expect("request should be recorded")
            .into_iter()
            .next()
            .expect("chat completion request should be recorded");

        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&request.body)
                .expect("request body should be valid JSON"),
            json!({
                "model": "sample-chat-model",
                "messages": [{
                    "role": "user",
                    "content": [
                        {"type": "text", "text": "summarize this sample"},
                        {
                            "type": "file",
                            "file": {
                                "file_id": "https://media.example.invalid/video",
                                "format": "video/mp4",
                            },
                        },
                    ],
                }],
            }),
        );
    }

    #[tokio::test]
    async fn chat_completion_maps_non_success_status_to_api_error() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(400).set_body_json(json!({
                "error": {"message": "video is private or unavailable"},
            })))
            .mount(&server)
            .await;

        let client = LiteLlmClient::new(&server.uri(), None).expect("build client");
        let err = client
            .chat_completion("sample-chat-model", vec![])
            .await
            .expect_err("expected error");
        assert!(matches!(
            err,
            LlmClientError::Api { status: 400, message } if message.contains("video is private or unavailable")
        ));
    }

    #[tokio::test]
    async fn chat_completion_errors_when_no_choices() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "choices": [] })))
            .mount(&server)
            .await;

        let client = LiteLlmClient::new(&server.uri(), None).expect("build client");
        let err = client
            .chat_completion("sample-chat-model", vec![])
            .await
            .expect_err("expected error");
        assert!(matches!(err, LlmClientError::Parse(_)));
    }
}
