use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use reqwest::StatusCode;
use reqwest::header::{HeaderMap, HeaderValue};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
const DEFAULT_CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, thiserror::Error)]
pub enum AgentTaskError {
    #[error("agent task client is not configured")]
    NotConfigured,

    #[error("agent task not found: {0}")]
    NotFound(String),

    #[error("agent task api error (status {status}): {message}")]
    Api { status: u16, message: String },

    #[error("network error: {0}")]
    Network(String),

    #[error("failed to parse response: {0}")]
    Parse(String),

    #[error("client initialization error: {0}")]
    Init(String),
}

/// t-rader-agent が `GET /internal/tasks/:task_id` で返す A2A TaskState (spec v0.3) の写し。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentTaskState {
    Submitted,
    Working,
    InputRequired,
    Completed,
    Canceled,
    Failed,
    Rejected,
}

impl AgentTaskState {
    fn from_raw(raw: &str) -> Option<Self> {
        match raw {
            "submitted" => Some(Self::Submitted),
            "working" => Some(Self::Working),
            "input-required" => Some(Self::InputRequired),
            "completed" => Some(Self::Completed),
            "canceled" => Some(Self::Canceled),
            "failed" => Some(Self::Failed),
            "rejected" => Some(Self::Rejected),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SubmitAgentTask {
    pub strategy_id: Uuid,
    pub prompt: String,
}

#[derive(Debug, Clone)]
pub struct AgentTaskRef {
    pub task_id: String,
}

#[derive(Debug, Clone)]
pub struct AgentTaskStatus {
    pub state: AgentTaskState,
    pub result_text: Option<String>,
    pub error_kind: Option<String>,
    /// フェーズ/分岐ごとの実行状況。中身は解釈せず素通しする。応答に無ければ `None`。
    pub steps: Option<serde_json::Value>,
}

#[async_trait]
pub trait AgentTaskClient: Send + Sync {
    async fn submit(&self, req: SubmitAgentTask) -> Result<AgentTaskRef, AgentTaskError>;

    async fn get(&self, task_id: &str) -> Result<AgentTaskStatus, AgentTaskError>;
}

/// 「無効化された」クライアント。すべての操作が `NotConfigured` を返す。
pub struct DisabledAgentTaskClient;

#[async_trait]
impl AgentTaskClient for DisabledAgentTaskClient {
    async fn submit(&self, _req: SubmitAgentTask) -> Result<AgentTaskRef, AgentTaskError> {
        Err(AgentTaskError::NotConfigured)
    }

    async fn get(&self, _task_id: &str) -> Result<AgentTaskStatus, AgentTaskError> {
        Err(AgentTaskError::NotConfigured)
    }
}

/// t-rader-agent の内部 API と通信するための設定
#[derive(Debug, Clone)]
pub struct AgentTaskClientConfig {
    /// t-rader-agent の内部 API base URL (例: `http://t-rader-agent.t-rader/internal`)
    pub api_base_url: String,
    /// 内部 API の bearer トークン
    pub bearer_token: String,
}

/// `TRADER_AGENT_API_URL` を opt-out 用に予約した特別値。dev 環境のみ想定。
pub const TRADER_AGENT_API_DISABLED_SENTINEL: &str = "disabled";

/// `from_env` の戻り値。production では `Configured` 必須、dev のみ `Disabled` を許容する。
#[derive(Debug)]
pub enum AgentTaskClientConfigSource {
    Configured(AgentTaskClientConfig),
    Disabled,
}

#[derive(Debug, thiserror::Error)]
pub enum AgentTaskClientConfigError {
    #[error(
        "TRADER_AGENT_API_URL is not set. Set it to the t-rader-agent internal API base URL, or to '{}' for explicit opt-out (dev only).",
        TRADER_AGENT_API_DISABLED_SENTINEL
    )]
    Missing,
    #[error("TRADER_AGENT_API_TOKEN is not set")]
    MissingToken,
}

impl AgentTaskClientConfig {
    /// 環境変数から設定を読み出す。`TRADER_AGENT_API_URL=disabled` は dev 用 opt-out。
    pub fn from_env() -> Result<AgentTaskClientConfigSource, AgentTaskClientConfigError> {
        Self::from_env_with(|key| std::env::var(key).ok())
    }

    fn from_env_with<F>(get: F) -> Result<AgentTaskClientConfigSource, AgentTaskClientConfigError>
    where
        F: Fn(&str) -> Option<String>,
    {
        let api_base_url = get("TRADER_AGENT_API_URL")
            .filter(|s| !s.is_empty())
            .ok_or(AgentTaskClientConfigError::Missing)?;
        if api_base_url == TRADER_AGENT_API_DISABLED_SENTINEL {
            return Ok(AgentTaskClientConfigSource::Disabled);
        }
        let bearer_token = get("TRADER_AGENT_API_TOKEN")
            .filter(|s| !s.is_empty())
            .ok_or(AgentTaskClientConfigError::MissingToken)?;
        Ok(AgentTaskClientConfigSource::Configured(Self {
            api_base_url,
            bearer_token,
        }))
    }
}

pub struct HttpAgentTaskClient {
    http: reqwest::Client,
    base_url: String,
}

impl HttpAgentTaskClient {
    pub fn new(config: AgentTaskClientConfig) -> Result<Self, AgentTaskError> {
        let mut headers = HeaderMap::new();
        let value = HeaderValue::from_str(&format!("Bearer {}", config.bearer_token))
            .map_err(|e| AgentTaskError::Init(format!("invalid bearer token: {e}")))?;
        headers.insert(reqwest::header::AUTHORIZATION, value);

        let http = reqwest::Client::builder()
            .timeout(DEFAULT_REQUEST_TIMEOUT)
            .connect_timeout(DEFAULT_CONNECT_TIMEOUT)
            .default_headers(headers)
            .build()
            .map_err(|e| AgentTaskError::Init(format!("failed to build http client: {e}")))?;

        Ok(Self {
            http,
            base_url: config.api_base_url.trim_end_matches('/').to_string(),
        })
    }

    fn tasks_url(&self) -> String {
        format!("{}/internal/tasks", self.base_url)
    }

    fn task_url(&self, task_id: &str) -> String {
        format!("{}/internal/tasks/{}", self.base_url, task_id)
    }
}

#[derive(Serialize)]
struct SubmitTaskBody<'a> {
    strategy_id: Uuid,
    prompt: &'a str,
}

#[derive(Deserialize)]
struct SubmitTaskResponse {
    task_id: String,
}

#[derive(Deserialize)]
struct GetTaskResponse {
    state: String,
    #[serde(default)]
    result_text: Option<String>,
    #[serde(default)]
    error_kind: Option<String>,
    #[serde(default)]
    steps: Option<serde_json::Value>,
}

#[derive(Deserialize)]
struct ApiErrorResponse {
    #[serde(default)]
    error: Option<String>,
}

fn api_error(status: StatusCode, body: String) -> AgentTaskError {
    let message = serde_json::from_str::<ApiErrorResponse>(&body)
        .ok()
        .and_then(|e| e.error)
        .unwrap_or(body);
    AgentTaskError::Api {
        status: status.as_u16(),
        message,
    }
}

#[async_trait]
impl AgentTaskClient for HttpAgentTaskClient {
    async fn submit(&self, req: SubmitAgentTask) -> Result<AgentTaskRef, AgentTaskError> {
        let response = self
            .http
            .post(self.tasks_url())
            .json(&SubmitTaskBody {
                strategy_id: req.strategy_id,
                prompt: &req.prompt,
            })
            .send()
            .await
            .map_err(|e| AgentTaskError::Network(e.to_string()))?;

        let status = response.status();
        if !status.is_success() {
            let text = response.text().await.unwrap_or_default();
            return Err(api_error(status, text));
        }
        let body: SubmitTaskResponse = response
            .json()
            .await
            .map_err(|e| AgentTaskError::Parse(e.to_string()))?;
        Ok(AgentTaskRef {
            task_id: body.task_id,
        })
    }

    async fn get(&self, task_id: &str) -> Result<AgentTaskStatus, AgentTaskError> {
        let response = self
            .http
            .get(self.task_url(task_id))
            .send()
            .await
            .map_err(|e| AgentTaskError::Network(e.to_string()))?;

        let status = response.status();
        if status == StatusCode::NOT_FOUND {
            return Err(AgentTaskError::NotFound(task_id.to_string()));
        }
        if !status.is_success() {
            let text = response.text().await.unwrap_or_default();
            return Err(api_error(status, text));
        }
        let body: GetTaskResponse = response
            .json()
            .await
            .map_err(|e| AgentTaskError::Parse(e.to_string()))?;
        let state = AgentTaskState::from_raw(&body.state).ok_or_else(|| {
            AgentTaskError::Parse(format!("unknown agent task state: {}", body.state))
        })?;
        Ok(AgentTaskStatus {
            state,
            result_text: body.result_text,
            error_kind: body.error_kind,
            steps: body.steps,
        })
    }
}

/// 共有用 alias。`Arc<dyn AgentTaskClient + Send + Sync>` を頻繁に書くのを避ける。
pub type SharedAgentTaskClient = Arc<dyn AgentTaskClient + Send + Sync>;

/// テスト向け: 受け取ったリクエストを記録して `Ok` または事前設定の応答を返すフェイク
#[cfg(test)]
#[derive(Default)]
pub struct FakeAgentTaskClient {
    pub submitted: tokio::sync::Mutex<Vec<SubmitAgentTask>>,
    pub statuses: tokio::sync::Mutex<std::collections::HashMap<String, AgentTaskStatus>>,
    pub next_task_id: tokio::sync::Mutex<Option<String>>,
    pub submit_error: tokio::sync::Mutex<Option<AgentTaskError>>,
    pub get_error: tokio::sync::Mutex<Option<AgentTaskError>>,
}

#[cfg(test)]
impl FakeAgentTaskClient {
    pub fn new() -> Self {
        Self::default()
    }
    pub async fn set_status(&self, task_id: &str, status: AgentTaskStatus) {
        self.statuses
            .lock()
            .await
            .insert(task_id.to_string(), status);
    }
    pub async fn set_next_task_id(&self, task_id: &str) {
        *self.next_task_id.lock().await = Some(task_id.to_string());
    }
    pub async fn set_submit_error(&self, err: AgentTaskError) {
        *self.submit_error.lock().await = Some(err);
    }
    pub async fn set_get_error(&self, err: AgentTaskError) {
        *self.get_error.lock().await = Some(err);
    }
}

#[cfg(test)]
#[async_trait]
impl AgentTaskClient for FakeAgentTaskClient {
    async fn submit(&self, req: SubmitAgentTask) -> Result<AgentTaskRef, AgentTaskError> {
        if let Some(err) = self.submit_error.lock().await.take() {
            return Err(err);
        }
        let task_id = self
            .next_task_id
            .lock()
            .await
            .take()
            .unwrap_or_else(|| format!("fake-task-{}", Uuid::new_v4()));
        self.submitted.lock().await.push(req);
        Ok(AgentTaskRef { task_id })
    }

    async fn get(&self, task_id: &str) -> Result<AgentTaskStatus, AgentTaskError> {
        if let Some(err) = self.get_error.lock().await.take() {
            return Err(err);
        }
        self.statuses
            .lock()
            .await
            .get(task_id)
            .cloned()
            .ok_or_else(|| AgentTaskError::NotFound(task_id.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;
    use serde_json::json;
    use wiremock::matchers::{body_json, header, method, path};
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

    fn expect_configured(source: AgentTaskClientConfigSource) -> AgentTaskClientConfig {
        match source {
            AgentTaskClientConfigSource::Configured(c) => c,
            AgentTaskClientConfigSource::Disabled => panic!("expected Configured, got Disabled"),
        }
    }

    #[rstest]
    #[case::missing_env(&[])]
    #[case::empty_env(&[("TRADER_AGENT_API_URL", "")])]
    fn from_env_invalid_api_url_fails_fast(#[case] env: &[(&str, &str)]) {
        let result = AgentTaskClientConfig::from_env_with(env_get(env));
        assert!(matches!(result, Err(AgentTaskClientConfigError::Missing)));
    }

    #[rstest]
    fn from_env_disabled_sentinel_returns_disabled() {
        let result = AgentTaskClientConfig::from_env_with(env_get(&[(
            "TRADER_AGENT_API_URL",
            TRADER_AGENT_API_DISABLED_SENTINEL,
        )]))
        .expect("disabled sentinel is accepted");
        assert!(matches!(result, AgentTaskClientConfigSource::Disabled));
    }

    #[rstest]
    fn from_env_missing_token_fails_fast() {
        let result = AgentTaskClientConfig::from_env_with(env_get(&[(
            "TRADER_AGENT_API_URL",
            "http://t-rader-agent/internal",
        )]));
        assert!(matches!(
            result,
            Err(AgentTaskClientConfigError::MissingToken)
        ));
    }

    #[rstest]
    fn from_env_with_url_and_token_returns_configured() {
        let config = expect_configured(
            AgentTaskClientConfig::from_env_with(env_get(&[
                ("TRADER_AGENT_API_URL", "http://t-rader-agent/internal"),
                ("TRADER_AGENT_API_TOKEN", "tok"),
            ]))
            .expect("configured"),
        );
        assert_eq!(config.api_base_url, "http://t-rader-agent/internal");
        assert_eq!(config.bearer_token, "tok");
    }

    #[rstest]
    #[case::submitted("submitted", Some(AgentTaskState::Submitted))]
    #[case::working("working", Some(AgentTaskState::Working))]
    #[case::input_required("input-required", Some(AgentTaskState::InputRequired))]
    #[case::completed("completed", Some(AgentTaskState::Completed))]
    #[case::canceled("canceled", Some(AgentTaskState::Canceled))]
    #[case::failed("failed", Some(AgentTaskState::Failed))]
    #[case::rejected("rejected", Some(AgentTaskState::Rejected))]
    #[case::unknown("mystery", None)]
    fn parses_agent_task_state(#[case] raw: &str, #[case] expected: Option<AgentTaskState>) {
        assert_eq!(AgentTaskState::from_raw(raw), expected);
    }

    fn http_client(server: &MockServer) -> HttpAgentTaskClient {
        HttpAgentTaskClient::new(AgentTaskClientConfig {
            api_base_url: server.uri(),
            bearer_token: "test-token".into(),
        })
        .expect("build client")
    }

    #[tokio::test]
    async fn submit_posts_body_and_returns_task_id() {
        let server = MockServer::start().await;
        let strategy_id = uuid::Uuid::parse_str("12345678-1234-5678-1234-567812345678").unwrap();

        Mock::given(method("POST"))
            .and(path("/internal/tasks"))
            .and(header("authorization", "Bearer test-token"))
            .and(body_json(json!({
                "strategy_id": strategy_id,
                "prompt": "hello",
            })))
            .respond_with(ResponseTemplate::new(201).set_body_json(json!({
                "task_id": "task-abc",
            })))
            .expect(1)
            .mount(&server)
            .await;

        let client = http_client(&server);
        let result = client
            .submit(SubmitAgentTask {
                strategy_id,
                prompt: "hello".into(),
            })
            .await
            .expect("submit ok");
        assert_eq!(result.task_id, "task-abc");
    }

    #[tokio::test]
    async fn submit_maps_error_response() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(422).set_body_json(json!({
                "error": "invalid request body",
            })))
            .mount(&server)
            .await;

        let client = http_client(&server);
        let err = client
            .submit(SubmitAgentTask {
                strategy_id: uuid::Uuid::new_v4(),
                prompt: "hello".into(),
            })
            .await
            .expect_err("expected error");
        assert_eq!(
            err.to_string(),
            "agent task api error (status 422): invalid request body"
        );
    }

    #[tokio::test]
    async fn get_returns_completed_status_with_result_text() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/internal/tasks/task-1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "task_id": "task-1",
                "state": "completed",
                "result_text": "done",
            })))
            .mount(&server)
            .await;

        let client = http_client(&server);
        let status = client.get("task-1").await.expect("ok");
        assert_eq!(status.state, AgentTaskState::Completed);
        assert_eq!(status.result_text.as_deref(), Some("done"));
        assert_eq!(status.error_kind, None);
        assert_eq!(status.steps, None);
    }

    #[tokio::test]
    async fn get_propagates_steps_when_present() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/internal/tasks/task-1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "task_id": "task-1",
                "state": "working",
                "steps": [{"phase_key": "example", "status": "running"}],
            })))
            .mount(&server)
            .await;

        let client = http_client(&server);
        let status = client.get("task-1").await.expect("ok");
        assert_eq!(
            status.steps,
            Some(json!([{"phase_key": "example", "status": "running"}])),
        );
    }

    #[tokio::test]
    async fn get_not_found_maps_to_not_found_error() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(404).set_body_json(json!({
                "error": "task not found",
            })))
            .mount(&server)
            .await;

        let client = http_client(&server);
        let err = client.get("missing").await.expect_err("expected error");
        assert_eq!(err.to_string(), "agent task not found: missing");
    }
}
