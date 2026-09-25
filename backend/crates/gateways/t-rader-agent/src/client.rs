use std::time::Duration;

use async_trait::async_trait;
use core_application::{
    AgentTaskClient, AgentTaskError, AgentTaskRef, AgentTaskState, AgentTaskStatus, SubmitAgentTask,
};
use reqwest::StatusCode;
use reqwest::header::{HeaderMap, HeaderValue};

use super::generated;

const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
const DEFAULT_CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

fn agent_task_state_from_raw(raw: &str) -> Option<AgentTaskState> {
    match raw {
        "submitted" => Some(AgentTaskState::Submitted),
        "working" => Some(AgentTaskState::Working),
        "input-required" => Some(AgentTaskState::InputRequired),
        "completed" => Some(AgentTaskState::Completed),
        "canceled" => Some(AgentTaskState::Canceled),
        "failed" => Some(AgentTaskState::Failed),
        "rejected" => Some(AgentTaskState::Rejected),
        _ => None,
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
    client: generated::Client,
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

        let base_url = config.api_base_url.trim_end_matches('/').to_string();
        Ok(Self {
            client: generated::Client::new_with_client(&base_url, http),
        })
    }
}

/// `ErrorResponse` は progenitor が全エラーレスポンスに共通で割り当てる型
/// (agent 側は 400/422/500/404 いずれも同じ `{error, issues?}` スキーマ)。
fn api_error(status: StatusCode, error: generated::types::ErrorResponse) -> AgentTaskError {
    AgentTaskError::Api {
        status: status.as_u16(),
        message: error.error,
    }
}

fn map_client_error(
    err: progenitor_client::Error<generated::types::ErrorResponse>,
) -> AgentTaskError {
    match err {
        progenitor_client::Error::ErrorResponse(rv) => {
            let status = rv.status();
            api_error(status, rv.into_inner())
        }
        progenitor_client::Error::CommunicationError(e) => AgentTaskError::Network(e.to_string()),
        progenitor_client::Error::InvalidUpgrade(e) => AgentTaskError::Network(e.to_string()),
        progenitor_client::Error::ResponseBodyError(e) => AgentTaskError::Parse(e.to_string()),
        progenitor_client::Error::InvalidResponsePayload(bytes, e) => {
            AgentTaskError::Parse(format!(
                "failed to parse error response body ({}): {e}",
                String::from_utf8_lossy(&bytes)
            ))
        }
        progenitor_client::Error::InvalidRequest(msg) | progenitor_client::Error::Custom(msg) => {
            AgentTaskError::Parse(msg)
        }
        progenitor_client::Error::UnexpectedResponse(response) => AgentTaskError::Api {
            status: response.status().as_u16(),
            message: format!("unexpected response status: {}", response.status()),
        },
    }
}

#[async_trait]
impl AgentTaskClient for HttpAgentTaskClient {
    async fn submit(&self, req: SubmitAgentTask) -> Result<AgentTaskRef, AgentTaskError> {
        let strategy_id =
            generated::types::SubmitTaskBodyStrategyId::try_from(req.strategy_id.to_string())
                .map_err(|e| AgentTaskError::Parse(format!("invalid strategy_id: {e}")))?;
        let prompt = generated::types::SubmitTaskBodyPrompt::try_from(req.prompt)
            .map_err(|e| AgentTaskError::Parse(format!("invalid prompt: {e}")))?;
        let purpose = req
            .purpose
            .map(generated::types::SubmitTaskBodyPurpose::try_from)
            .transpose()
            .map_err(|e| AgentTaskError::Parse(format!("invalid purpose: {e}")))?;
        let body = generated::types::SubmitTaskBody {
            strategy_id,
            prompt,
            purpose,
            resume_steps: req.resume_steps.unwrap_or_default(),
            deadline_at: Some(req.deadline_at.with_timezone(&chrono::Utc)),
            as_of: req.as_of.map(|t| t.with_timezone(&chrono::Utc)),
        };

        let response = self
            .client
            .submit_task(&body)
            .await
            .map_err(map_client_error)?;
        Ok(AgentTaskRef {
            task_id: response.into_inner().task_id,
        })
    }

    async fn get(&self, task_id: &str) -> Result<AgentTaskStatus, AgentTaskError> {
        let response = self.client.get_task(task_id).await.map_err(|e| {
            if let progenitor_client::Error::ErrorResponse(ref rv) = e
                && rv.status() == StatusCode::NOT_FOUND
            {
                return AgentTaskError::NotFound(task_id.to_string());
            }
            map_client_error(e)
        })?;

        let body = response.into_inner();
        let state = agent_task_state_from_raw(&body.state).ok_or_else(|| {
            AgentTaskError::Parse(format!("unknown agent task state: {}", body.state))
        })?;
        Ok(AgentTaskStatus {
            state,
            result_text: body.result_text,
            error_message: body.error_message,
            error_kind: body.error_kind,
            steps: (!body.steps.is_empty()).then_some(serde_json::Value::Array(body.steps)),
        })
    }
}

#[cfg(test)]
#[path = "client_tests.rs"]
mod tests;
