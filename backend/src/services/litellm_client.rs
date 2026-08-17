//! LiteLLM Proxy の `/model_group/info` を素通しするクライアント
//!
//! backend が agent 設定フォームにモデル選択肢を供給するための薄いプロキシ。
//! LiteLLM の API キーを frontend に晒さないためだけの中継で、キャッシュはしない。

use std::time::Duration;

use serde::Deserialize;

use crate::models::AgentModel;

const REQUEST_TIMEOUT: Duration = Duration::from_secs(8);

#[derive(Debug, thiserror::Error)]
pub enum LiteLlmError {
    #[error("network error: {0}")]
    Network(String),

    #[error("litellm api error (status {0})")]
    Api(u16),

    #[error("failed to parse response: {0}")]
    Parse(String),

    #[error("client initialization error: {0}")]
    Init(String),
}

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
    #[serde(default)]
    supports_web_search: bool,
}

impl From<ModelGroupInfo> for AgentModel {
    fn from(m: ModelGroupInfo) -> Self {
        Self {
            id: m.model_group,
            providers: m.providers,
            max_input_tokens: m.max_input_tokens,
            max_output_tokens: m.max_output_tokens,
            supports_reasoning: m.supports_reasoning,
            supports_web_search: m.supports_web_search,
        }
    }
}

/// LiteLLM Proxy client。`LLM_BASE_URL` 未設定の環境 (ローカル開発で OpenCode Go に
/// フォールバックしている場合等) では `from_env` が `None` を返すので、呼び出し元は
/// そのまま「モデル一覧なし」として扱えばよい。
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
            tracing::warn!(
                "LLM_BASE_URL が未設定のため、litellm client を無効化します (GET /api/agent-models は空配列を返す)"
            );
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

    pub(crate) fn new(base_url: &str, api_key: Option<&str>) -> Result<Self, LiteLlmError> {
        let mut headers = reqwest::header::HeaderMap::new();
        if let Some(key) = api_key {
            let value = reqwest::header::HeaderValue::from_str(&format!("Bearer {key}"))
                .map_err(|e| LiteLlmError::Init(format!("invalid api key: {e}")))?;
            headers.insert(reqwest::header::AUTHORIZATION, value);
        }
        let http = reqwest::Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .default_headers(headers)
            .build()
            .map_err(|e| LiteLlmError::Init(format!("failed to build http client: {e}")))?;

        // OpenAI 互換の `LLM_BASE_URL` (例: `http://litellm/v1`) と、LiteLLM 管理系 API の
        // ベース URL は別物。`/model_group/info` は `/v1` の下ではなくルート直下にある。
        let trimmed = base_url.trim_end_matches('/');
        let base_url = trimmed.strip_suffix("/v1").unwrap_or(trimmed).to_string();

        Ok(Self { http, base_url })
    }

    pub async fn list_models(&self) -> Result<Vec<AgentModel>, LiteLlmError> {
        let url = format!("{}/model_group/info", self.base_url);
        let response = self
            .http
            .get(url)
            .send()
            .await
            .map_err(|e| LiteLlmError::Network(e.to_string()))?;

        let status = response.status();
        if !status.is_success() {
            return Err(LiteLlmError::Api(status.as_u16()));
        }

        let body: ModelGroupInfoResponse = response
            .json()
            .await
            .map_err(|e| LiteLlmError::Parse(e.to_string()))?;
        Ok(body.data.into_iter().map(AgentModel::from).collect())
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
                        "model_group": "claude-opus-4",
                        "providers": ["anthropic"],
                        "max_input_tokens": 200000.0,
                        "max_output_tokens": 8192.0,
                        "supports_reasoning": true,
                        "supports_web_search": false,
                    },
                ],
            })))
            .mount(&server)
            .await;

        let client = LiteLlmClient::new(&server.uri(), None).expect("build client");
        let models = client.list_models().await.expect("list models ok");
        assert_eq!(
            models,
            vec![AgentModel {
                id: "claude-opus-4".to_string(),
                providers: vec!["anthropic".to_string()],
                max_input_tokens: Some(200000.0),
                max_output_tokens: Some(8192.0),
                supports_reasoning: true,
                supports_web_search: false,
            }],
        );
    }

    #[tokio::test]
    async fn list_models_maps_non_success_status_to_api_error() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/model_group/info"))
            .respond_with(ResponseTemplate::new(503))
            .mount(&server)
            .await;

        let client = LiteLlmClient::new(&server.uri(), None).expect("build client");
        let err = client.list_models().await.expect_err("expected error");
        assert!(matches!(err, LiteLlmError::Api(503)));
    }
}
