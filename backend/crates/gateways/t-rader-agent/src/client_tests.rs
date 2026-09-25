use chrono::{DateTime, FixedOffset};
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
    assert_eq!(agent_task_state_from_raw(raw), expected);
}

fn http_client(server: &MockServer) -> HttpAgentTaskClient {
    HttpAgentTaskClient::new(AgentTaskClientConfig {
        api_base_url: server.uri(),
        bearer_token: "test-token".into(),
    })
    .expect("build client")
}

fn test_deadline_at() -> DateTime<FixedOffset> {
    chrono::DateTime::parse_from_rfc3339("2020-01-01T00:00:00Z").unwrap()
}

/// `test_deadline_at` と取り違えを検出できるよう、別の値にする。
fn test_as_of() -> DateTime<FixedOffset> {
    chrono::DateTime::parse_from_rfc3339("2019-06-01T12:34:56Z").unwrap()
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
            "deadline_at": "2020-01-01T00:00:00Z",
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
            purpose: None,
            resume_steps: None,
            deadline_at: test_deadline_at(),
            as_of: None,
        })
        .await
        .expect("submit ok");
    assert_eq!(result.task_id, "task-abc");
}

#[rstest]
#[case::none(None, json!({ "strategy_id": "12345678-1234-5678-1234-567812345678", "prompt": "hello", "deadline_at": "2020-01-01T00:00:00Z" }))]
#[case::some(Some("explore".to_string()), json!({ "strategy_id": "12345678-1234-5678-1234-567812345678", "prompt": "hello", "purpose": "explore", "deadline_at": "2020-01-01T00:00:00Z" }))]
#[tokio::test]
async fn submit_body_includes_purpose_only_when_some(
    #[case] purpose: Option<String>,
    #[case] expected_body: serde_json::Value,
) {
    let server = MockServer::start().await;
    let strategy_id = uuid::Uuid::parse_str("12345678-1234-5678-1234-567812345678").unwrap();

    Mock::given(method("POST"))
        .and(path("/internal/tasks"))
        .and(body_json(expected_body))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({
            "task_id": "task-abc",
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = http_client(&server);
    client
        .submit(SubmitAgentTask {
            strategy_id,
            prompt: "hello".into(),
            purpose,
            resume_steps: None,
            deadline_at: test_deadline_at(),
            as_of: None,
        })
        .await
        .expect("submit ok");
}

#[rstest]
#[case::none(None, json!({ "strategy_id": "12345678-1234-5678-1234-567812345678", "prompt": "hello", "deadline_at": "2020-01-01T00:00:00Z" }))]
#[case::some(Some(test_as_of()), json!({ "strategy_id": "12345678-1234-5678-1234-567812345678", "prompt": "hello", "deadline_at": "2020-01-01T00:00:00Z", "as_of": "2019-06-01T12:34:56Z" }))]
#[tokio::test]
async fn submit_body_includes_as_of_only_when_some(
    #[case] as_of: Option<DateTime<FixedOffset>>,
    #[case] expected_body: serde_json::Value,
) {
    let server = MockServer::start().await;
    let strategy_id = uuid::Uuid::parse_str("12345678-1234-5678-1234-567812345678").unwrap();

    Mock::given(method("POST"))
        .and(path("/internal/tasks"))
        .and(body_json(expected_body))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({
            "task_id": "task-abc",
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = http_client(&server);
    client
        .submit(SubmitAgentTask {
            strategy_id,
            prompt: "hello".into(),
            purpose: None,
            resume_steps: None,
            deadline_at: test_deadline_at(),
            as_of,
        })
        .await
        .expect("submit ok");
}

#[rstest]
#[case::none(None, json!({ "strategy_id": "12345678-1234-5678-1234-567812345678", "prompt": "hello", "deadline_at": "2020-01-01T00:00:00Z" }))]
#[case::some(
        Some(vec![json!({ "execution_step_id": "step-1", "status": "completed" })]),
        json!({
            "strategy_id": "12345678-1234-5678-1234-567812345678",
            "prompt": "hello",
            "resume_steps": [{ "execution_step_id": "step-1", "status": "completed" }],
            "deadline_at": "2020-01-01T00:00:00Z",
        })
    )]
#[tokio::test]
async fn submit_body_includes_resume_steps_only_when_some(
    #[case] resume_steps: Option<Vec<serde_json::Value>>,
    #[case] expected_body: serde_json::Value,
) {
    let server = MockServer::start().await;
    let strategy_id = uuid::Uuid::parse_str("12345678-1234-5678-1234-567812345678").unwrap();

    Mock::given(method("POST"))
        .and(path("/internal/tasks"))
        .and(body_json(expected_body))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({
            "task_id": "task-abc",
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = http_client(&server);
    client
        .submit(SubmitAgentTask {
            strategy_id,
            prompt: "hello".into(),
            purpose: None,
            resume_steps,
            deadline_at: test_deadline_at(),
            as_of: None,
        })
        .await
        .expect("submit ok");
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
            purpose: None,
            resume_steps: None,
            deadline_at: test_deadline_at(),
            as_of: None,
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
async fn get_returns_failed_status_with_error_message_and_kind() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/internal/tasks/task-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "task_id": "task-1",
            "state": "failed",
            "error_message": "フェーズ「調査」(investigate) の実行に失敗しました: boom",
            "error_kind": "agent_error",
        })))
        .mount(&server)
        .await;

    let client = http_client(&server);
    let status = client.get("task-1").await.expect("ok");
    assert_eq!(
        (status.state, status.error_message, status.error_kind),
        (
            AgentTaskState::Failed,
            Some("フェーズ「調査」(investigate) の実行に失敗しました: boom".to_string()),
            Some("agent_error".to_string()),
        ),
    );
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
