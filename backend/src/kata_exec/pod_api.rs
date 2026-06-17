use std::time::Duration;

use async_trait::async_trait;
use reqwest::{Certificate, StatusCode, header::HeaderMap, header::HeaderValue};
use serde::Deserialize;

use super::config::KataExecutorConfig;
use super::error::KataExecError;
use super::manifest::{PodPhase, PodStatusInfo};

const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
const DEFAULT_CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

const SA_TOKEN_PATH: &str = "/var/run/secrets/kubernetes.io/serviceaccount/token";
const SA_CA_PATH: &str = "/var/run/secrets/kubernetes.io/serviceaccount/ca.crt";

/// kube-apiserver の Pod リソース操作を 1 枚に抽象化する trait。
///
/// `HttpKataExecutor` はこの trait 越しに Pod を操作するため、テストは reqwest を
/// 触らず in-memory な mock 実装で動かせる。
#[async_trait]
pub(crate) trait PodApi: Send + Sync {
    async fn create(&self, manifest: &serde_json::Value) -> Result<(), KataExecError>;
    async fn get_status(&self, name: &str) -> Result<PodStatusInfo, KataExecError>;
    async fn fetch_log(&self, name: &str, max_bytes: usize) -> Result<String, KataExecError>;
    async fn delete(&self, name: &str);
    /// Drop 経路 (sync コンテキスト) からの fire-and-forget な削除。
    /// 実装側で `tokio::spawn` する前提。
    fn spawn_delete(&self, name: &str);
}

pub(crate) struct ReqwestPodApi {
    http: reqwest::Client,
    pods_url: String,
}

impl ReqwestPodApi {
    pub(crate) fn new(config: &KataExecutorConfig) -> Result<Self, KataExecError> {
        let mut builder = reqwest::Client::builder()
            .timeout(DEFAULT_REQUEST_TIMEOUT)
            .connect_timeout(DEFAULT_CONNECT_TIMEOUT);

        let token = match &config.bearer_token {
            Some(token) => Some(token.clone()),
            None => read_optional_file(SA_TOKEN_PATH)?,
        };
        if let Some(token) = token {
            let mut headers = HeaderMap::new();
            let value = HeaderValue::from_str(&format!("Bearer {token}"))
                .map_err(|e| KataExecError::Init(format!("invalid bearer token: {e}")))?;
            headers.insert(reqwest::header::AUTHORIZATION, value);
            builder = builder.default_headers(headers);
        }

        if config.insecure_tls {
            builder = builder.danger_accept_invalid_certs(true);
        } else {
            let ca_path = config.ca_cert_path.clone().or_else(|| {
                if std::path::Path::new(SA_CA_PATH).exists() {
                    Some(SA_CA_PATH.to_string())
                } else {
                    None
                }
            });
            if let Some(ca_path) = ca_path {
                let pem = std::fs::read(&ca_path).map_err(|e| {
                    KataExecError::Init(format!("failed to read CA cert {ca_path}: {e}"))
                })?;
                let cert = Certificate::from_pem(&pem)
                    .map_err(|e| KataExecError::Init(format!("failed to parse CA cert: {e}")))?;
                builder = builder.add_root_certificate(cert);
            }
        }

        let http = builder
            .build()
            .map_err(|e| KataExecError::Init(format!("failed to build http client: {e}")))?;
        let pods_url = format!(
            "{base}/api/v1/namespaces/{ns}/pods",
            base = config.api_base_url.trim_end_matches('/'),
            ns = config.namespace,
        );
        Ok(Self { http, pods_url })
    }

    fn pod_url(&self, name: &str) -> String {
        format!("{}/{}", self.pods_url, name)
    }

    fn pod_log_url(&self, name: &str) -> String {
        format!("{}/log", self.pod_url(name))
    }

    fn delete_url(&self, name: &str) -> String {
        format!("{}?gracePeriodSeconds=0", self.pod_url(name))
    }

    /// `delete` (sync) と `spawn_delete` (Drop 経由 fire-and-forget) の共通実装。
    /// 404 = 既に削除済みは正常扱い。ログ文言だけ context で出し分ける。
    async fn run_delete(http: &reqwest::Client, url: &str, pod_name: &str, context: &str) {
        match http.delete(url).send().await {
            Ok(res) => {
                let status = res.status();
                if !status.is_success() && status != StatusCode::NOT_FOUND {
                    tracing::warn!(pod = pod_name, %status, "{}", context);
                }
            }
            Err(e) => {
                tracing::warn!(pod = pod_name, error = %e, "{} (network error)", context);
            }
        }
    }
}

#[async_trait]
impl PodApi for ReqwestPodApi {
    async fn create(&self, manifest: &serde_json::Value) -> Result<(), KataExecError> {
        let response = self
            .http
            .post(&self.pods_url)
            .json(manifest)
            .send()
            .await
            .map_err(|e| KataExecError::Network(e.to_string()))?;
        let status = response.status();
        if status.is_success() {
            return Ok(());
        }
        let text = response.text().await.unwrap_or_default();
        Err(api_error(status, text))
    }

    async fn get_status(&self, name: &str) -> Result<PodStatusInfo, KataExecError> {
        let response = self
            .http
            .get(self.pod_url(name))
            .send()
            .await
            .map_err(|e| KataExecError::Network(e.to_string()))?;
        let status = response.status();
        if !status.is_success() {
            let text = response.text().await.unwrap_or_default();
            return Err(api_error(status, text));
        }
        let body: PodResponse = response
            .json()
            .await
            .map_err(|e| KataExecError::Parse(e.to_string()))?;
        Ok(pod_status_info(body.status.unwrap_or_default()))
    }

    async fn fetch_log(&self, name: &str, max_bytes: usize) -> Result<String, KataExecError> {
        // `limitBytes` ぴったり = "上限ちょうど" を OK 扱いするため 1 byte 多く要求し、
        // 戻り値が `> max_bytes` のときだけ OutputTooLarge とする。
        let url = format!("{}?limitBytes={}", self.pod_log_url(name), max_bytes + 1);
        let response = self
            .http
            .get(&url)
            .send()
            .await
            .map_err(|e| KataExecError::Network(e.to_string()))?;
        let status = response.status();
        if !status.is_success() {
            let text = response.text().await.unwrap_or_default();
            return Err(api_error(status, text));
        }
        // limitBytes は UTF-8 境界を考慮しないので、bytes で受けて lossy にデコードする。
        let body = response
            .bytes()
            .await
            .map_err(|e| KataExecError::Network(e.to_string()))?;
        if body.len() > max_bytes {
            return Err(KataExecError::OutputTooLarge { limit: max_bytes });
        }
        Ok(String::from_utf8_lossy(&body).into_owned())
    }

    async fn delete(&self, name: &str) {
        Self::run_delete(
            &self.http,
            &self.delete_url(name),
            name,
            "kata exec pod delete returned non-success",
        )
        .await;
    }

    fn spawn_delete(&self, name: &str) {
        let http = self.http.clone();
        let url = self.delete_url(name);
        let pod_name = name.to_string();
        tokio::spawn(async move {
            Self::run_delete(
                &http,
                &url,
                &pod_name,
                "kata exec pod delete on drop returned non-success",
            )
            .await;
        });
    }
}

fn read_optional_file(path: &str) -> Result<Option<String>, KataExecError> {
    match std::fs::read_to_string(path) {
        Ok(s) => Ok(Some(s.trim().to_string())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(KataExecError::Init(format!("failed to read {path}: {e}"))),
    }
}

fn api_error(status: StatusCode, body: String) -> KataExecError {
    let message = serde_json::from_str::<K8sStatusResponse>(&body)
        .ok()
        .and_then(|s| s.message.or(s.reason))
        .unwrap_or(body);
    KataExecError::Api {
        status: status.as_u16(),
        message,
    }
}

fn pod_status_info(s: PodResponseStatus) -> PodStatusInfo {
    let phase = s
        .phase
        .as_deref()
        .map(PodPhase::from_raw)
        .unwrap_or(PodPhase::Unknown);
    let terminated_reason = s
        .container_statuses
        .into_iter()
        .find_map(|cs| cs.state.and_then(|st| st.terminated).and_then(|t| t.reason));
    PodStatusInfo {
        phase,
        terminated_reason,
        message: s.message,
    }
}

#[derive(Debug, Default, Deserialize)]
struct PodResponse {
    #[serde(default)]
    status: Option<PodResponseStatus>,
}

#[derive(Debug, Default, Deserialize)]
struct PodResponseStatus {
    #[serde(default)]
    phase: Option<String>,
    #[serde(default)]
    message: Option<String>,
    #[serde(default, rename = "containerStatuses")]
    container_statuses: Vec<ContainerStatus>,
}

#[derive(Debug, Default, Deserialize)]
struct ContainerStatus {
    #[serde(default)]
    state: Option<ContainerState>,
}

#[derive(Debug, Default, Deserialize)]
struct ContainerState {
    #[serde(default)]
    terminated: Option<ContainerStateTerminated>,
}

#[derive(Debug, Default, Deserialize)]
struct ContainerStateTerminated {
    #[serde(default)]
    reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct K8sStatusResponse {
    #[serde(default)]
    message: Option<String>,
    #[serde(default)]
    reason: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::super::config::PodResourceLimits;
    use super::*;
    use serde_json::json;
    use wiremock::matchers::{method, path, path_regex};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn config_for(server: &MockServer) -> KataExecutorConfig {
        KataExecutorConfig {
            api_base_url: server.uri(),
            namespace: "t-rader-exec".into(),
            image: "ghcr.io/fohte/t-rader/python-exec:latest".into(),
            bearer_token: Some("test-token".into()),
            ca_cert_path: None,
            insecure_tls: true,
            default_timeout: Duration::from_millis(500),
            default_max_output_bytes: 1024,
            resource_limits: PodResourceLimits::default(),
            poll_interval: Duration::from_millis(10),
        }
    }

    /// ReqwestPodApi が kube-apiserver の REST endpoint を正しい path/method で叩き、
    /// 応答を `PodStatusInfo` / log 文字列に詰め直すラウンドトリップを検証する。
    /// オーケストレーション側 (HttpKataExecutor) のテストは MockPodApi で済むので、
    /// wiremock を立てる integration test はこの 1 本に集約している。
    #[tokio::test]
    async fn reqwest_pod_api_round_trip() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/v1/namespaces/t-rader-exec/pods"))
            .respond_with(ResponseTemplate::new(201).set_body_json(json!({})))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path_regex(r"^/api/v1/namespaces/t-rader-exec/pods/[^/]+$"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "status": { "phase": "Succeeded" },
            })))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path_regex(
                r"^/api/v1/namespaces/t-rader-exec/pods/[^/]+/log$",
            ))
            .respond_with(ResponseTemplate::new(200).set_body_string("hello"))
            .mount(&server)
            .await;
        Mock::given(method("DELETE"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;

        let api = ReqwestPodApi::new(&config_for(&server)).expect("build api");
        api.create(&json!({})).await.expect("create");
        let status = api.get_status("test-pod").await.expect("status");
        assert_eq!(status.phase, PodPhase::Succeeded);
        let log = api.fetch_log("test-pod", 1024).await.expect("log");
        assert_eq!(log, "hello");
        api.delete("test-pod").await;
    }

    #[tokio::test]
    async fn fetch_log_rejects_when_body_exceeds_max_bytes() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path_regex(
                r"^/api/v1/namespaces/t-rader-exec/pods/[^/]+/log$",
            ))
            .respond_with(ResponseTemplate::new(200).set_body_string("x".repeat(2048)))
            .mount(&server)
            .await;

        let api = ReqwestPodApi::new(&config_for(&server)).expect("build api");
        let err = api
            .fetch_log("test-pod", 1024)
            .await
            .expect_err("expected OutputTooLarge");
        assert_eq!(err, KataExecError::OutputTooLarge { limit: 1024 });
    }
}
