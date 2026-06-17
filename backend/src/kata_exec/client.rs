use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;
use reqwest::{Certificate, StatusCode, header::HeaderMap, header::HeaderValue};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
const DEFAULT_CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const DEFAULT_POLL_INTERVAL: Duration = Duration::from_millis(500);
const DEFAULT_WALL_CLOCK_TIMEOUT: Duration = Duration::from_secs(60);
const DEFAULT_MAX_OUTPUT_BYTES: usize = 1024 * 1024;
const DEFAULT_CPU_LIMIT: &str = "500m";
const DEFAULT_MEMORY_LIMIT: &str = "256Mi";
const DEFAULT_EPHEMERAL_STORAGE_LIMIT: &str = "64Mi";
const DEFAULT_NAMESPACE: &str = "t-rader-exec";
const DEFAULT_IMAGE: &str = "ghcr.io/fohte/t-rader/python-exec:latest";
const DEFAULT_POD_NAME_PREFIX: &str = "t-rader-exec";
const RUNTIME_CLASS_NAME: &str = "kata";
const ENVELOPE_MARKER: &str = "__T_RADER_ENVELOPE__";

const SA_TOKEN_PATH: &str = "/var/run/secrets/kubernetes.io/serviceaccount/token";
const SA_CA_PATH: &str = "/var/run/secrets/kubernetes.io/serviceaccount/ca.crt";

#[derive(Debug, thiserror::Error)]
pub enum KataExecError {
    #[error("kata executor is not configured")]
    NotConfigured,

    #[error("execution timed out after {0:?}")]
    Timeout(Duration),

    #[error("output exceeded {limit} bytes")]
    OutputTooLarge { limit: usize },

    #[error("kube api error (status {status}): {message}")]
    Api { status: u16, message: String },

    #[error("network error: {0}")]
    Network(String),

    #[error("failed to parse response: {0}")]
    Parse(String),

    #[error("exec pod terminated abnormally: {0}")]
    PodFailed(String),

    #[error("client initialization error: {0}")]
    Init(String),
}

/// Pod の CPU / memory / ephemeral-storage 上限
#[derive(Debug, Clone)]
pub struct PodResourceLimits {
    pub cpu: String,
    pub memory: String,
    pub ephemeral_storage: String,
}

impl Default for PodResourceLimits {
    fn default() -> Self {
        Self {
            cpu: DEFAULT_CPU_LIMIT.to_string(),
            memory: DEFAULT_MEMORY_LIMIT.to_string(),
            ephemeral_storage: DEFAULT_EPHEMERAL_STORAGE_LIMIT.to_string(),
        }
    }
}

/// Python コードを 1 回実行するためのリクエスト
#[derive(Debug, Clone)]
pub struct ExecRequest {
    /// 実行する Python コード本体 (utf-8)
    pub code: String,
    /// 実行中に Python の sys.stdin に流す入力 (省略可)
    pub stdin: Option<String>,
    /// Pod 全体の wall-clock 上限 (backend 側で時計を見る)。
    /// 同時に Pod 側の `activeDeadlineSeconds` も同じ値を入れる。
    pub timeout: Option<Duration>,
    /// stdout + stderr の合計バイト数の上限。超えた場合 `OutputTooLarge` を返す。
    pub max_output_bytes: Option<usize>,
}

impl ExecRequest {
    pub fn new(code: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            stdin: None,
            timeout: None,
            max_output_bytes: None,
        }
    }
}

/// Pod の実行結果
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecResult {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

#[async_trait]
pub trait KataExecutor: Send + Sync {
    async fn run(&self, request: ExecRequest) -> Result<ExecResult, KataExecError>;
}

pub type SharedKataExecutor = Arc<dyn KataExecutor + Send + Sync>;

/// 「無効化された」executor。すべての操作が `NotConfigured` を返す。
pub struct DisabledKataExecutor;

#[async_trait]
impl KataExecutor for DisabledKataExecutor {
    async fn run(&self, _request: ExecRequest) -> Result<ExecResult, KataExecError> {
        Err(KataExecError::NotConfigured)
    }
}

#[derive(Debug, Clone)]
pub struct KataExecutorConfig {
    /// kube-apiserver の base URL (例: `https://kubernetes.default.svc`)
    pub api_base_url: String,
    /// exec Pod を作成する namespace
    pub namespace: String,
    /// 実行に使う Python image
    pub image: String,
    /// Bearer トークン (省略時は ServiceAccount の token を読む)
    pub bearer_token: Option<String>,
    /// 追加 CA 証明書のパス (省略時は ServiceAccount の ca.crt を読む)
    pub ca_cert_path: Option<String>,
    /// テスト用: TLS 検証をスキップする
    pub insecure_tls: bool,
    /// デフォルトの wall-clock timeout
    pub default_timeout: Duration,
    /// デフォルトの出力サイズ上限
    pub default_max_output_bytes: usize,
    /// Pod の resource limits
    pub resource_limits: PodResourceLimits,
    /// Pod の status を polling する間隔
    pub poll_interval: Duration,
}

impl KataExecutorConfig {
    /// 環境変数から設定を読み出す。`KATA_EXEC_API_URL` が未設定なら `None`。
    pub fn from_env() -> Option<Self> {
        let api_base_url = std::env::var("KATA_EXEC_API_URL")
            .ok()
            .filter(|s| !s.is_empty())?;
        let namespace = std::env::var("KATA_EXEC_NAMESPACE")
            .ok()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| DEFAULT_NAMESPACE.to_string());
        let image = std::env::var("KATA_EXEC_IMAGE")
            .ok()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| DEFAULT_IMAGE.to_string());
        let bearer_token = std::env::var("KATA_EXEC_TOKEN")
            .ok()
            .filter(|s| !s.is_empty());
        let ca_cert_path = std::env::var("KATA_EXEC_CA_CERT_PATH")
            .ok()
            .filter(|s| !s.is_empty());
        let insecure_tls = std::env::var("KATA_EXEC_INSECURE_TLS")
            .ok()
            .map(|v| matches!(v.to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on"))
            .unwrap_or(false);
        let default_timeout = std::env::var("KATA_EXEC_DEFAULT_TIMEOUT_SECS")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .map(Duration::from_secs)
            .unwrap_or(DEFAULT_WALL_CLOCK_TIMEOUT);
        let default_max_output_bytes = std::env::var("KATA_EXEC_MAX_OUTPUT_BYTES")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(DEFAULT_MAX_OUTPUT_BYTES);

        Some(Self {
            api_base_url,
            namespace,
            image,
            bearer_token,
            ca_cert_path,
            insecure_tls,
            default_timeout,
            default_max_output_bytes,
            resource_limits: PodResourceLimits::default(),
            poll_interval: DEFAULT_POLL_INTERVAL,
        })
    }
}

/// `run` の Future がキャンセル (ドロップ) された時に、tokio::spawn で
/// fire-and-forget の DELETE を発行して Pod を回収するガード。
/// 正常パスでは `disarm()` を呼び、同期 `delete_pod` 経由で結果を捕捉する。
struct PodDeleteGuard {
    http: reqwest::Client,
    delete_url: String,
    pod_name: String,
    armed: bool,
}

impl PodDeleteGuard {
    fn new(http: reqwest::Client, delete_url: String, pod_name: String) -> Self {
        Self {
            http,
            delete_url,
            pod_name,
            armed: true,
        }
    }

    fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for PodDeleteGuard {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        let http = self.http.clone();
        let url = self.delete_url.clone();
        let pod_name = std::mem::take(&mut self.pod_name);
        tokio::spawn(async move {
            match http.delete(&url).send().await {
                Ok(res) => {
                    let status = res.status();
                    if !status.is_success() && status != StatusCode::NOT_FOUND {
                        tracing::warn!(
                            pod = %pod_name,
                            %status,
                            "kata exec pod delete on drop returned non-success",
                        );
                    }
                }
                Err(e) => {
                    tracing::warn!(
                        pod = %pod_name,
                        error = %e,
                        "kata exec pod delete on drop failed",
                    );
                }
            }
        });
    }
}

pub struct HttpKataExecutor {
    http: reqwest::Client,
    api_base_url: String,
    namespace: String,
    image: String,
    default_timeout: Duration,
    default_max_output_bytes: usize,
    resource_limits: PodResourceLimits,
    poll_interval: Duration,
}

impl HttpKataExecutor {
    pub fn new(config: KataExecutorConfig) -> Result<Self, KataExecError> {
        let mut builder = reqwest::Client::builder()
            .timeout(DEFAULT_REQUEST_TIMEOUT)
            .connect_timeout(DEFAULT_CONNECT_TIMEOUT);

        let token = match config.bearer_token {
            Some(token) => Some(token),
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
            let ca_path = config.ca_cert_path.or_else(|| {
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
        Ok(Self {
            http,
            api_base_url: config.api_base_url.trim_end_matches('/').to_string(),
            namespace: config.namespace,
            image: config.image,
            default_timeout: config.default_timeout,
            default_max_output_bytes: config.default_max_output_bytes,
            resource_limits: config.resource_limits,
            poll_interval: config.poll_interval,
        })
    }

    fn pods_url(&self) -> String {
        format!(
            "{base}/api/v1/namespaces/{ns}/pods",
            base = self.api_base_url,
            ns = self.namespace,
        )
    }

    fn pod_url(&self, name: &str) -> String {
        format!("{}/{}", self.pods_url(), name)
    }

    fn pod_log_url(&self, name: &str) -> String {
        format!("{}/log", self.pod_url(name))
    }

    async fn create_pod(&self, manifest: &serde_json::Value) -> Result<(), KataExecError> {
        let response = self
            .http
            .post(self.pods_url())
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

    async fn get_pod_phase(&self, name: &str) -> Result<PodStatusInfo, KataExecError> {
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
        Ok(PodStatusInfo::from(body.status.unwrap_or_default()))
    }

    async fn get_pod_log(&self, name: &str, max_bytes: usize) -> Result<String, KataExecError> {
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

    async fn delete_pod(&self, name: &str) {
        let url = format!("{}?gracePeriodSeconds=0", self.pod_url(name));
        match self.http.delete(&url).send().await {
            Ok(res) => {
                let status = res.status();
                // 404 = 既に削除済み (Drop guard と通常パスで二重削除されるケース) は正常扱い。
                if !status.is_success() && status != StatusCode::NOT_FOUND {
                    tracing::warn!(pod = name, %status, "kata exec pod delete returned non-success");
                }
            }
            Err(e) => {
                tracing::warn!(pod = name, error = %e, "failed to delete kata exec pod");
            }
        }
    }

    async fn wait_for_terminal(
        &self,
        pod_name: &str,
        max_output_bytes: usize,
    ) -> Result<ExecResult, KataExecError> {
        loop {
            let info = self.get_pod_phase(pod_name).await?;
            match info.phase {
                PodPhase::Succeeded | PodPhase::Failed => {
                    let logs = self.get_pod_log(pod_name, max_output_bytes).await?;
                    return assemble_result(&logs, info);
                }
                PodPhase::Pending | PodPhase::Running | PodPhase::Unknown => {
                    tokio::time::sleep(self.poll_interval).await;
                }
            }
        }
    }
}

#[async_trait]
impl KataExecutor for HttpKataExecutor {
    async fn run(&self, request: ExecRequest) -> Result<ExecResult, KataExecError> {
        let wall_clock_timeout = request.timeout.unwrap_or(self.default_timeout);
        let max_output_bytes = request
            .max_output_bytes
            .unwrap_or(self.default_max_output_bytes);
        let pod_name = generate_pod_name();
        let manifest = build_pod_manifest(
            &pod_name,
            &self.namespace,
            &self.image,
            &request,
            &self.resource_limits,
            wall_clock_timeout.as_secs().max(1) as i64,
        );

        tracing::debug!(pod = %pod_name, "creating kata exec pod");
        if let Err(e) = self.create_pod(&manifest).await {
            tracing::warn!(pod = %pod_name, error = %e, "failed to create kata exec pod");
            return Err(e);
        }

        // 呼び出し元から run の Future がドロップされても Pod を確実に消すため、
        // tokio::spawn による fire-and-forget の DELETE を Drop で発行する。
        let mut guard = PodDeleteGuard::new(
            self.http.clone(),
            format!("{}?gracePeriodSeconds=0", self.pod_url(&pod_name)),
            pod_name.clone(),
        );

        let outcome = tokio::time::timeout(
            wall_clock_timeout,
            self.wait_for_terminal(&pod_name, max_output_bytes),
        )
        .await;
        // 正常パスでは同期的に削除し、結果をログに残す。guard 側の fire-and-forget は冗長になる。
        guard.disarm();
        self.delete_pod(&pod_name).await;

        match outcome {
            Ok(Ok(result)) => Ok(result),
            Ok(Err(e)) => {
                tracing::warn!(pod = %pod_name, error = %e, "kata exec pod failed");
                Err(e)
            }
            Err(_) => {
                tracing::warn!(
                    pod = %pod_name,
                    timeout_secs = wall_clock_timeout.as_secs(),
                    "kata exec pod hit wall-clock timeout",
                );
                Err(KataExecError::Timeout(wall_clock_timeout))
            }
        }
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

fn generate_pod_name() -> String {
    let id = Uuid::new_v4().simple().to_string();
    let truncated: String = id.chars().take(16).collect();
    format!("{DEFAULT_POD_NAME_PREFIX}-{truncated}")
}

/// 実行結果を log から組み立てる。
///
/// entrypoint が JSON envelope を最終行に印字する設計なので、envelope を見つけて parse する。
/// envelope が見つからない場合 (Pod が壊れた、image が古い 等) は `PodFailed` として扱う。
fn assemble_result(logs: &str, info: PodStatusInfo) -> Result<ExecResult, KataExecError> {
    // 最終行から marker を探す。LLM が生成したコードがマーカー文字列を出力しても
    // entrypoint の最終 envelope が常に最後の出現になるよう rsplit_once を使う。
    if let Some((_pre, envelope_payload)) = logs.rsplit_once(ENVELOPE_MARKER) {
        let envelope_json = envelope_payload.trim_start_matches(['\n', '\r']);
        let envelope: ExecEnvelope = serde_json::from_str(envelope_json)
            .map_err(|e| KataExecError::Parse(format!("failed to parse exec envelope: {e}")))?;
        return Ok(ExecResult {
            stdout: envelope.stdout,
            stderr: envelope.stderr,
            exit_code: envelope.exit_code,
        });
    }

    // envelope なし: container が起動失敗 / OOMKilled / activeDeadline で殺された等。
    let reason = info.terminated_reason.or(info.message).unwrap_or_else(|| {
        if logs.is_empty() {
            "no output from exec pod".to_string()
        } else {
            logs.to_string()
        }
    });
    Err(KataExecError::PodFailed(reason))
}

/// Pod manifest を組み立てる。
///
/// 不変条件: `runtimeClassName: kata`, `automountServiceAccountToken: false`,
/// `activeDeadlineSeconds`, `restartPolicy: Never`, 1 container, resource limits,
/// non-root + read-only-rootfs + drop ALL caps.
pub(crate) fn build_pod_manifest(
    pod_name: &str,
    namespace: &str,
    image: &str,
    request: &ExecRequest,
    limits: &PodResourceLimits,
    active_deadline_seconds: i64,
) -> serde_json::Value {
    let code_b64 = BASE64.encode(request.code.as_bytes());
    let mut env = vec![json!({
        "name": "EXEC_CODE_B64",
        "value": code_b64,
    })];
    if let Some(stdin) = &request.stdin {
        env.push(json!({
            "name": "EXEC_STDIN_B64",
            "value": BASE64.encode(stdin.as_bytes()),
        }));
    }

    json!({
        "apiVersion": "v1",
        "kind": "Pod",
        "metadata": {
            "name": pod_name,
            "namespace": namespace,
            "labels": {
                "app.kubernetes.io/name": "t-rader-exec",
                "app.kubernetes.io/managed-by": "t-rader-backend",
            },
        },
        "spec": {
            "runtimeClassName": RUNTIME_CLASS_NAME,
            "automountServiceAccountToken": false,
            "restartPolicy": "Never",
            "activeDeadlineSeconds": active_deadline_seconds,
            "enableServiceLinks": false,
            "securityContext": {
                "runAsNonRoot": true,
                "runAsUser": 65532,
                "runAsGroup": 65532,
                "seccompProfile": { "type": "RuntimeDefault" },
            },
            "containers": [{
                "name": "exec",
                "image": image,
                "imagePullPolicy": "IfNotPresent",
                "env": env,
                "resources": {
                    "limits": {
                        "cpu": limits.cpu,
                        "memory": limits.memory,
                        "ephemeral-storage": limits.ephemeral_storage,
                    },
                    "requests": {
                        "cpu": limits.cpu,
                        "memory": limits.memory,
                        "ephemeral-storage": limits.ephemeral_storage,
                    },
                },
                "securityContext": {
                    "allowPrivilegeEscalation": false,
                    "readOnlyRootFilesystem": true,
                    "capabilities": { "drop": ["ALL"] },
                },
                "volumeMounts": [{
                    "name": "tmp",
                    "mountPath": "/tmp",
                }],
            }],
            "volumes": [{
                "name": "tmp",
                "emptyDir": { "medium": "Memory", "sizeLimit": "16Mi" },
            }],
        },
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PodPhase {
    Pending,
    Running,
    Succeeded,
    Failed,
    Unknown,
}

impl PodPhase {
    fn from_raw(raw: &str) -> Self {
        match raw {
            "Pending" => Self::Pending,
            "Running" => Self::Running,
            "Succeeded" => Self::Succeeded,
            "Failed" => Self::Failed,
            _ => Self::Unknown,
        }
    }
}

#[derive(Debug, Clone)]
struct PodStatusInfo {
    phase: PodPhase,
    terminated_reason: Option<String>,
    message: Option<String>,
}

impl From<PodResponseStatus> for PodStatusInfo {
    fn from(s: PodResponseStatus) -> Self {
        let phase = s
            .phase
            .as_deref()
            .map(PodPhase::from_raw)
            .unwrap_or(PodPhase::Unknown);
        let terminated_reason = s
            .container_statuses
            .into_iter()
            .find_map(|cs| cs.state.and_then(|st| st.terminated).and_then(|t| t.reason));
        Self {
            phase,
            terminated_reason,
            message: s.message,
        }
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
struct ExecEnvelope {
    stdout: String,
    stderr: String,
    exit_code: i32,
}

#[derive(Debug, Deserialize)]
struct K8sStatusResponse {
    #[serde(default)]
    message: Option<String>,
    #[serde(default)]
    reason: Option<String>,
}

/// テスト向け: 受け取った request を記録して事前設定の応答を返すフェイク
#[cfg(test)]
#[derive(Default)]
pub struct FakeKataExecutor {
    pub requests: tokio::sync::Mutex<Vec<ExecRequest>>,
    pub response: tokio::sync::Mutex<Option<Result<ExecResult, KataExecError>>>,
}

#[cfg(test)]
impl FakeKataExecutor {
    pub fn new() -> Self {
        Self::default()
    }
    pub async fn set_response(&self, response: Result<ExecResult, KataExecError>) {
        *self.response.lock().await = Some(response);
    }
}

#[cfg(test)]
#[async_trait]
impl KataExecutor for FakeKataExecutor {
    async fn run(&self, request: ExecRequest) -> Result<ExecResult, KataExecError> {
        self.requests.lock().await.push(request);
        match self.response.lock().await.take() {
            Some(r) => r,
            None => Ok(ExecResult {
                stdout: String::new(),
                stderr: String::new(),
                exit_code: 0,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;
    use serde_json::json;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn manifest_for(code: &str, stdin: Option<&str>) -> serde_json::Value {
        build_pod_manifest(
            "t-rader-exec-abcdef",
            "t-rader-exec",
            "ghcr.io/fohte/t-rader/python-exec:latest",
            &ExecRequest {
                code: code.to_string(),
                stdin: stdin.map(str::to_string),
                timeout: None,
                max_output_bytes: None,
            },
            &PodResourceLimits::default(),
            30,
        )
    }

    #[rstest]
    fn builds_pod_manifest_matches_required_invariants() {
        let m = manifest_for("print(1+1)", None);
        assert_eq!(
            m,
            json!({
                "apiVersion": "v1",
                "kind": "Pod",
                "metadata": {
                    "name": "t-rader-exec-abcdef",
                    "namespace": "t-rader-exec",
                    "labels": {
                        "app.kubernetes.io/name": "t-rader-exec",
                        "app.kubernetes.io/managed-by": "t-rader-backend",
                    },
                },
                "spec": {
                    "runtimeClassName": "kata",
                    "automountServiceAccountToken": false,
                    "restartPolicy": "Never",
                    "activeDeadlineSeconds": 30,
                    "enableServiceLinks": false,
                    "securityContext": {
                        "runAsNonRoot": true,
                        "runAsUser": 65532,
                        "runAsGroup": 65532,
                        "seccompProfile": { "type": "RuntimeDefault" },
                    },
                    "containers": [{
                        "name": "exec",
                        "image": "ghcr.io/fohte/t-rader/python-exec:latest",
                        "imagePullPolicy": "IfNotPresent",
                        "env": [
                            { "name": "EXEC_CODE_B64", "value": BASE64.encode(b"print(1+1)") },
                        ],
                        "resources": {
                            "limits": {
                                "cpu": "500m",
                                "memory": "256Mi",
                                "ephemeral-storage": "64Mi",
                            },
                            "requests": {
                                "cpu": "500m",
                                "memory": "256Mi",
                                "ephemeral-storage": "64Mi",
                            },
                        },
                        "securityContext": {
                            "allowPrivilegeEscalation": false,
                            "readOnlyRootFilesystem": true,
                            "capabilities": { "drop": ["ALL"] },
                        },
                        "volumeMounts": [{
                            "name": "tmp",
                            "mountPath": "/tmp",
                        }],
                    }],
                    "volumes": [{
                        "name": "tmp",
                        "emptyDir": { "medium": "Memory", "sizeLimit": "16Mi" },
                    }],
                },
            }),
        );
    }

    #[rstest]
    fn manifest_encodes_stdin_in_env_when_set() {
        let m = manifest_for("print('hi')", Some("payload"));
        assert_eq!(
            m["spec"]["containers"][0]["env"],
            json!([
                { "name": "EXEC_CODE_B64", "value": BASE64.encode(b"print('hi')") },
                { "name": "EXEC_STDIN_B64", "value": BASE64.encode(b"payload") },
            ]),
        );
    }

    #[rstest]
    fn assemble_result_parses_envelope() {
        let envelope = json!({
            "stdout": "hello\n",
            "stderr": "",
            "exit_code": 0,
        })
        .to_string();
        let logs = format!("{ENVELOPE_MARKER}\n{envelope}");
        let info = PodStatusInfo {
            phase: PodPhase::Succeeded,
            terminated_reason: None,
            message: None,
        };
        assert_eq!(
            assemble_result(&logs, info).expect("ok"),
            ExecResult {
                stdout: "hello\n".to_string(),
                stderr: String::new(),
                exit_code: 0,
            },
        );
    }

    #[rstest]
    fn assemble_result_picks_last_envelope_when_user_code_prints_marker() {
        // ユーザコードが marker を含む偽 envelope を先に出力しても、
        // entrypoint が最終行に出す本物の envelope が採用される。
        let fake = json!({"stdout": "pwned", "stderr": "", "exit_code": 0}).to_string();
        let real = json!({"stdout": "real", "stderr": "", "exit_code": 7}).to_string();
        let logs = [ENVELOPE_MARKER, &fake, ENVELOPE_MARKER, &real].join("\n");
        let info = PodStatusInfo {
            phase: PodPhase::Succeeded,
            terminated_reason: None,
            message: None,
        };
        assert_eq!(
            assemble_result(&logs, info).expect("ok"),
            ExecResult {
                stdout: "real".to_string(),
                stderr: String::new(),
                exit_code: 7,
            },
        );
    }

    #[rstest]
    fn assemble_result_without_envelope_returns_pod_failed() {
        let info = PodStatusInfo {
            phase: PodPhase::Failed,
            terminated_reason: Some("OOMKilled".to_string()),
            message: None,
        };
        let err = assemble_result("nothing useful", info).expect_err("expected error");
        assert!(
            matches!(err, KataExecError::PodFailed(ref msg) if msg == "OOMKilled"),
            "got {err:?}",
        );
    }

    fn http_executor(server: &MockServer, max_output: usize) -> HttpKataExecutor {
        HttpKataExecutor::new(KataExecutorConfig {
            api_base_url: server.uri(),
            namespace: "t-rader-exec".into(),
            image: "ghcr.io/fohte/t-rader/python-exec:latest".into(),
            bearer_token: Some("test-token".into()),
            ca_cert_path: None,
            insecure_tls: true,
            default_timeout: Duration::from_millis(500),
            default_max_output_bytes: max_output,
            resource_limits: PodResourceLimits::default(),
            poll_interval: Duration::from_millis(10),
        })
        .expect("build executor")
    }

    fn pod_status_body(phase: &str) -> serde_json::Value {
        json!({ "status": { "phase": phase } })
    }

    fn envelope_log(stdout: &str, stderr: &str, exit_code: i32) -> String {
        let env = json!({
            "stdout": stdout,
            "stderr": stderr,
            "exit_code": exit_code,
        });
        format!("{ENVELOPE_MARKER}\n{env}")
    }

    use wiremock::matchers::path_regex;

    /// POST pod, GET phase, GET log, DELETE のライフサイクル mock を一括登録する。
    async fn mount_pod_lifecycle(server: &MockServer, phase: &str, log_body: String) {
        Mock::given(method("POST"))
            .and(path("/api/v1/namespaces/t-rader-exec/pods"))
            .respond_with(ResponseTemplate::new(201).set_body_json(json!({})))
            .mount(server)
            .await;
        Mock::given(method("GET"))
            .and(path_regex(r"^/api/v1/namespaces/t-rader-exec/pods/[^/]+$"))
            .respond_with(ResponseTemplate::new(200).set_body_json(pod_status_body(phase)))
            .mount(server)
            .await;
        Mock::given(method("GET"))
            .and(path_regex(
                r"^/api/v1/namespaces/t-rader-exec/pods/[^/]+/log$",
            ))
            .respond_with(ResponseTemplate::new(200).set_body_string(log_body))
            .mount(server)
            .await;
        Mock::given(method("DELETE"))
            .respond_with(ResponseTemplate::new(200))
            .mount(server)
            .await;
    }

    #[tokio::test]
    async fn run_returns_exec_result_with_logs_envelope() {
        let server = MockServer::start().await;
        mount_pod_lifecycle(&server, "Succeeded", envelope_log("2\n", "", 0)).await;
        let exec = http_executor(&server, 1024);
        assert_eq!(
            exec.run(ExecRequest::new("print(1+1)")).await.expect("ok"),
            ExecResult {
                stdout: "2\n".to_string(),
                stderr: String::new(),
                exit_code: 0,
            },
        );
    }

    #[tokio::test]
    async fn run_times_out_when_pod_stays_running() {
        let server = MockServer::start().await;
        mount_pod_lifecycle(&server, "Running", String::new()).await;

        let exec = http_executor(&server, 1024);
        let mut req = ExecRequest::new("while True: pass");
        req.timeout = Some(Duration::from_millis(100));
        let err = exec.run(req).await.expect_err("expected timeout");
        assert!(matches!(err, KataExecError::Timeout(_)), "got {err:?}");
    }

    #[tokio::test]
    async fn run_rejects_oversized_output() {
        let server = MockServer::start().await;
        mount_pod_lifecycle(&server, "Succeeded", "x".repeat(2048)).await;

        let exec = http_executor(&server, 1024);
        let err = exec
            .run(ExecRequest::new("print('x' * 5000)"))
            .await
            .expect_err("expected OutputTooLarge");
        assert!(
            matches!(err, KataExecError::OutputTooLarge { limit: 1024 }),
            "got {err:?}",
        );
    }

    #[tokio::test]
    async fn run_future_drop_triggers_pod_delete() {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicUsize, Ordering};

        let server = MockServer::start().await;
        let delete_calls = Arc::new(AtomicUsize::new(0));

        Mock::given(method("POST"))
            .and(path("/api/v1/namespaces/t-rader-exec/pods"))
            .respond_with(ResponseTemplate::new(201).set_body_json(json!({})))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path_regex(r"^/api/v1/namespaces/t-rader-exec/pods/[^/]+$"))
            // Pod がずっと Running を返すので、wait_for_terminal は終わらない。
            .respond_with(ResponseTemplate::new(200).set_body_json(pod_status_body("Running")))
            .mount(&server)
            .await;

        let counter = delete_calls.clone();
        Mock::given(method("DELETE"))
            .respond_with(move |_: &wiremock::Request| {
                counter.fetch_add(1, Ordering::SeqCst);
                ResponseTemplate::new(200)
            })
            .mount(&server)
            .await;

        let exec = Arc::new(http_executor(&server, 1024));
        let exec_for_task = exec.clone();
        let mut req = ExecRequest::new("while True: pass");
        req.timeout = Some(Duration::from_secs(10));

        // 呼び出し側で future を tokio::time::timeout で打ち切ってドロップする。
        // Drop guard が DELETE を spawn する経路を踏ませる。
        let _ = tokio::time::timeout(Duration::from_millis(50), async move {
            exec_for_task.run(req).await
        })
        .await;

        // tokio::spawn された delete が走るまで少し待つ。
        for _ in 0..50 {
            if delete_calls.load(Ordering::SeqCst) > 0 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        assert!(
            delete_calls.load(Ordering::SeqCst) >= 1,
            "DELETE was not invoked after run() future was dropped",
        );
    }
}
