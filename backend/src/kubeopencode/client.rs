use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use reqwest::{Certificate, StatusCode, header::HeaderMap, header::HeaderValue};
use serde::Deserialize;
use serde_json::json;

use crate::kubeopencode::manifest::{
    AGENT_GROUP, AGENT_PLURAL, AGENT_VERSION, AgentOwnerRef, CONFIGMAP_PLURAL, DEFAULT_AGENT_MODEL,
    DEFAULT_AGENT_SMALL_MODEL, DEFAULT_SSM_PARAMETER_TEMPLATE, EXTERNAL_SECRET_GROUP,
    EXTERNAL_SECRET_PLURAL, EXTERNAL_SECRET_VERSION, FIELD_MANAGER, SA_PLURAL,
    StrategyAgentSettings, StrategyAgentSpec, build_agent_manifest, build_external_secret_manifest,
    build_service_account_manifest, build_workspace_configmap_manifest,
};

const TASK_CR_GROUP: &str = "kubeopencode.io";
const TASK_CR_VERSION: &str = "v1alpha1";
const TASK_CR_PLURAL: &str = "tasks";

const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
const DEFAULT_CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

const DEFAULT_NAMESPACE: &str = "kubeopencode";
const SA_TOKEN_PATH: &str = "/var/run/secrets/kubernetes.io/serviceaccount/token";
const SA_CA_PATH: &str = "/var/run/secrets/kubernetes.io/serviceaccount/ca.crt";

#[derive(Debug, thiserror::Error)]
pub enum KubeopencodeError {
    #[error("kubeopencode is not configured")]
    NotConfigured,

    #[error("task already exists: {0}")]
    AlreadyExists(String),

    #[error("task not found: {0}")]
    NotFound(String),

    #[error("kubeopencode api error (status {status}): {message}")]
    Api { status: u16, message: String },

    #[error("network error: {0}")]
    Network(String),

    #[error("failed to parse response: {0}")]
    Parse(String),

    #[error("client initialization error: {0}")]
    Init(String),
}

/// kubeopencode Task の phase 値 (kubeopencode が `status.phase` で返す文字列を小文字化したもの)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskPhase {
    Pending,
    Running,
    Completed,
    Failed,
}

impl TaskPhase {
    pub fn from_raw(raw: &str) -> Option<Self> {
        match raw.to_ascii_lowercase().as_str() {
            "pending" => Some(Self::Pending),
            "running" => Some(Self::Running),
            "completed" | "succeeded" => Some(Self::Completed),
            "failed" => Some(Self::Failed),
            _ => None,
        }
    }
}

/// kubeopencode の Task CR `status` から抜き出した情報
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskCrStatus {
    pub phase: Option<TaskPhase>,
    pub message: Option<String>,
}

/// Task CR を作成する際の指定
#[derive(Debug, Clone)]
pub struct TaskCrSpec {
    pub name: String,
    pub agent_name: String,
    pub description: String,
}

#[async_trait]
pub trait KubeopencodeClient: Send + Sync {
    async fn create_task(&self, spec: &TaskCrSpec) -> Result<(), KubeopencodeError>;

    async fn get_task_status(&self, name: &str) -> Result<TaskCrStatus, KubeopencodeError>;

    /// 戦略 Agent CR と関連リソース (ServiceAccount / ConfigMap / ExternalSecret) を冪等に apply する。
    ///
    /// 同じ spec で繰り返し呼ばれることを前提とし、Server-Side Apply で field manager
    /// `t-rader-backend` の所有 field を毎回上書きする。SA / ConfigMap / ExternalSecret は
    /// Agent CR を ownerReference に持ち、Agent 削除で cascade GC される。
    async fn reconcile_strategy_agent(
        &self,
        spec: &StrategyAgentSpec,
    ) -> Result<(), KubeopencodeError>;

    /// 戦略 Agent CR を削除する。下流リソースは ownerReference 経由で operator が GC する。
    /// 既に消えている場合は成功扱い。
    async fn delete_strategy_agent(&self, agent_name: &str) -> Result<(), KubeopencodeError>;
}

/// 「無効化された」クライアント。すべての操作が `NotConfigured` を返す。
pub struct DisabledKubeopencodeClient;

#[async_trait]
impl KubeopencodeClient for DisabledKubeopencodeClient {
    async fn create_task(&self, _spec: &TaskCrSpec) -> Result<(), KubeopencodeError> {
        Err(KubeopencodeError::NotConfigured)
    }

    async fn get_task_status(&self, _name: &str) -> Result<TaskCrStatus, KubeopencodeError> {
        Err(KubeopencodeError::NotConfigured)
    }

    async fn reconcile_strategy_agent(
        &self,
        _spec: &StrategyAgentSpec,
    ) -> Result<(), KubeopencodeError> {
        Err(KubeopencodeError::NotConfigured)
    }

    async fn delete_strategy_agent(&self, _agent_name: &str) -> Result<(), KubeopencodeError> {
        Err(KubeopencodeError::NotConfigured)
    }
}

/// kubeopencode (実体は kube-apiserver) と通信するための設定
#[derive(Debug, Clone)]
pub struct KubeopencodeConfig {
    /// kube-apiserver の base URL (例: `https://kubernetes.default.svc`)
    pub api_base_url: String,
    pub namespace: String,
    /// Bearer トークン (省略時は ServiceAccount の token を読む)
    pub bearer_token: Option<String>,
    /// 追加 CA 証明書のパス (省略時は ServiceAccount の ca.crt を読む)
    pub ca_cert_path: Option<String>,
    /// テスト用: TLS 検証をスキップする
    pub insecure_tls: bool,
    /// 戦略 Agent reconcile 用の設定
    pub strategy_agent: StrategyAgentSettings,
}

/// `KUBEOPENCODE_API_URL` を opt-out 用に予約した特別値。dev 環境のみ想定。
pub const KUBEOPENCODE_DISABLED_SENTINEL: &str = "disabled";

/// `from_env` の戻り値。production では `Configured` 必須、dev のみ `Disabled` を許容する。
#[derive(Debug)]
pub enum KubeopencodeConfigSource {
    Configured(KubeopencodeConfig),
    Disabled,
}

#[derive(Debug, thiserror::Error)]
pub enum KubeopencodeConfigError {
    #[error(
        "KUBEOPENCODE_API_URL is not set. Set it to the kube-apiserver URL, or to '{}' for explicit opt-out (dev only).",
        KUBEOPENCODE_DISABLED_SENTINEL
    )]
    Missing,
    #[error(
        "STRATEGY_MCP_URL is not set. Set it to the t-rader-backend strategy MCP endpoint (e.g. http://t-rader-backend.t-rader/mcp/strategy)."
    )]
    MissingStrategyMcpUrl,
}

impl KubeopencodeConfig {
    /// 環境変数から設定を読み出す。`KUBEOPENCODE_API_URL=disabled` は dev 用 opt-out。
    pub fn from_env() -> Result<KubeopencodeConfigSource, KubeopencodeConfigError> {
        Self::from_env_with(|key| std::env::var(key).ok())
    }

    fn from_env_with<F>(get: F) -> Result<KubeopencodeConfigSource, KubeopencodeConfigError>
    where
        F: Fn(&str) -> Option<String>,
    {
        let api_base_url = get("KUBEOPENCODE_API_URL")
            .filter(|s| !s.is_empty())
            .ok_or(KubeopencodeConfigError::Missing)?;
        if api_base_url == KUBEOPENCODE_DISABLED_SENTINEL {
            return Ok(KubeopencodeConfigSource::Disabled);
        }
        let namespace = get("KUBEOPENCODE_NAMESPACE")
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| DEFAULT_NAMESPACE.to_string());
        let bearer_token = get("KUBEOPENCODE_TOKEN").filter(|s| !s.is_empty());
        let ca_cert_path = get("KUBEOPENCODE_CA_CERT_PATH").filter(|s| !s.is_empty());
        let insecure_tls = get("KUBEOPENCODE_INSECURE_TLS")
            .map(|v| matches!(v.as_str(), "1" | "true" | "TRUE"))
            .unwrap_or(false);
        let mcp_url = get("STRATEGY_MCP_URL")
            .filter(|s| !s.is_empty())
            .ok_or(KubeopencodeConfigError::MissingStrategyMcpUrl)?;
        let ssm_parameter_template = get("STRATEGY_AGENT_SSM_PARAMETER_TEMPLATE")
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| DEFAULT_SSM_PARAMETER_TEMPLATE.to_string());
        let model = get("STRATEGY_AGENT_MODEL")
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| DEFAULT_AGENT_MODEL.to_string());
        let small_model = get("STRATEGY_AGENT_SMALL_MODEL")
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| DEFAULT_AGENT_SMALL_MODEL.to_string());
        let strategy_agent = StrategyAgentSettings {
            mcp_url,
            ssm_parameter_template,
            model,
            small_model,
        };
        Ok(KubeopencodeConfigSource::Configured(Self {
            api_base_url,
            namespace,
            bearer_token,
            ca_cert_path,
            insecure_tls,
            strategy_agent,
        }))
    }
}

pub struct HttpKubeopencodeClient {
    http: reqwest::Client,
    api_base_url: String,
    namespace: String,
    strategy_agent: StrategyAgentSettings,
}

impl HttpKubeopencodeClient {
    pub fn new(config: KubeopencodeConfig) -> Result<Self, KubeopencodeError> {
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
                .map_err(|e| KubeopencodeError::Init(format!("invalid bearer token: {e}")))?;
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
                    KubeopencodeError::Init(format!("failed to read CA cert {ca_path}: {e}"))
                })?;
                let cert = Certificate::from_pem(&pem).map_err(|e| {
                    KubeopencodeError::Init(format!("failed to parse CA cert: {e}"))
                })?;
                builder = builder.add_root_certificate(cert);
            }
        }

        let http = builder
            .build()
            .map_err(|e| KubeopencodeError::Init(format!("failed to build http client: {e}")))?;
        Ok(Self {
            http,
            api_base_url: config.api_base_url.trim_end_matches('/').to_string(),
            namespace: config.namespace,
            strategy_agent: config.strategy_agent,
        })
    }

    fn collection_url(&self) -> String {
        format!(
            "{base}/apis/{group}/{version}/namespaces/{ns}/{plural}",
            base = self.api_base_url,
            group = TASK_CR_GROUP,
            version = TASK_CR_VERSION,
            ns = self.namespace,
            plural = TASK_CR_PLURAL,
        )
    }

    fn item_url(&self, name: &str) -> String {
        format!("{}/{}", self.collection_url(), name)
    }

    /// Server-Side Apply 用の URL を組み立てる。
    fn apply_url(&self, namespaced_path: &str, name: &str) -> String {
        format!(
            "{base}{path}/{name}?fieldManager={fm}&force=true",
            base = self.api_base_url,
            path = namespaced_path,
            name = name,
            fm = FIELD_MANAGER,
        )
    }

    /// Agent CR の SSA URL
    fn agent_apply_url(&self, name: &str) -> String {
        let path = format!(
            "/apis/{group}/{version}/namespaces/{ns}/{plural}",
            group = AGENT_GROUP,
            version = AGENT_VERSION,
            ns = self.namespace,
            plural = AGENT_PLURAL,
        );
        self.apply_url(&path, name)
    }

    fn agent_item_url(&self, name: &str) -> String {
        format!(
            "{base}/apis/{group}/{version}/namespaces/{ns}/{plural}/{name}",
            base = self.api_base_url,
            group = AGENT_GROUP,
            version = AGENT_VERSION,
            ns = self.namespace,
            plural = AGENT_PLURAL,
            name = name,
        )
    }

    fn service_account_apply_url(&self, name: &str) -> String {
        let path = format!(
            "/api/v1/namespaces/{ns}/{plural}",
            ns = self.namespace,
            plural = SA_PLURAL,
        );
        self.apply_url(&path, name)
    }

    fn configmap_apply_url(&self, name: &str) -> String {
        let path = format!(
            "/api/v1/namespaces/{ns}/{plural}",
            ns = self.namespace,
            plural = CONFIGMAP_PLURAL,
        );
        self.apply_url(&path, name)
    }

    fn external_secret_apply_url(&self, name: &str) -> String {
        let path = format!(
            "/apis/{group}/{version}/namespaces/{ns}/{plural}",
            group = EXTERNAL_SECRET_GROUP,
            version = EXTERNAL_SECRET_VERSION,
            ns = self.namespace,
            plural = EXTERNAL_SECRET_PLURAL,
        );
        self.apply_url(&path, name)
    }

    /// 1 マニフェストを SSA で apply する。レスポンス本文 (生成済みの metadata を含む) を返す。
    async fn apply_manifest(
        &self,
        url: &str,
        manifest: &serde_json::Value,
    ) -> Result<serde_json::Value, KubeopencodeError> {
        let response = self
            .http
            .patch(url)
            .header(
                reqwest::header::CONTENT_TYPE,
                "application/apply-patch+yaml",
            )
            .json(manifest)
            .send()
            .await
            .map_err(|e| KubeopencodeError::Network(e.to_string()))?;

        let status = response.status();
        if status.is_success() {
            return response
                .json::<serde_json::Value>()
                .await
                .map_err(|e| KubeopencodeError::Parse(e.to_string()));
        }
        let text = response.text().await.unwrap_or_default();
        Err(api_error(status, text))
    }
}

fn read_optional_file(path: &str) -> Result<Option<String>, KubeopencodeError> {
    match std::fs::read_to_string(path) {
        Ok(s) => Ok(Some(s.trim().to_string())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(KubeopencodeError::Init(format!(
            "failed to read {path}: {e}"
        ))),
    }
}

#[derive(Debug, Deserialize)]
struct TaskCrResponse {
    #[serde(default)]
    status: Option<TaskCrResponseStatus>,
}

#[derive(Debug, Deserialize)]
struct TaskCrResponseStatus {
    #[serde(default)]
    phase: Option<String>,
    #[serde(default)]
    message: Option<String>,
}

#[derive(Debug, Deserialize)]
struct K8sStatusResponse {
    #[serde(default)]
    message: Option<String>,
    #[serde(default)]
    reason: Option<String>,
}

#[async_trait]
impl KubeopencodeClient for HttpKubeopencodeClient {
    async fn create_task(&self, spec: &TaskCrSpec) -> Result<(), KubeopencodeError> {
        let body = build_task_cr_manifest(spec, &self.namespace);
        let response = self
            .http
            .post(self.collection_url())
            .json(&body)
            .send()
            .await
            .map_err(|e| KubeopencodeError::Network(e.to_string()))?;

        let status = response.status();
        if status.is_success() {
            return Ok(());
        }
        let text = response.text().await.unwrap_or_default();
        if status == StatusCode::CONFLICT {
            return Err(KubeopencodeError::AlreadyExists(spec.name.clone()));
        }
        Err(api_error(status, text))
    }

    async fn get_task_status(&self, name: &str) -> Result<TaskCrStatus, KubeopencodeError> {
        let response = self
            .http
            .get(self.item_url(name))
            .send()
            .await
            .map_err(|e| KubeopencodeError::Network(e.to_string()))?;

        let status_code = response.status();
        if status_code == StatusCode::NOT_FOUND {
            return Err(KubeopencodeError::NotFound(name.to_string()));
        }
        if !status_code.is_success() {
            let text = response.text().await.unwrap_or_default();
            return Err(api_error(status_code, text));
        }
        let body: TaskCrResponse = response
            .json()
            .await
            .map_err(|e| KubeopencodeError::Parse(e.to_string()))?;
        let (phase, message) = match body.status {
            Some(s) => (s.phase.as_deref().and_then(TaskPhase::from_raw), s.message),
            None => (None, None),
        };
        Ok(TaskCrStatus { phase, message })
    }

    async fn reconcile_strategy_agent(
        &self,
        spec: &StrategyAgentSpec,
    ) -> Result<(), KubeopencodeError> {
        // 1. Agent CR を SSA。レスポンスから UID を取り出し、子リソースに ownerReference を張る。
        let agent_manifest = build_agent_manifest(spec, &self.strategy_agent, &self.namespace);
        let agent_response = self
            .apply_manifest(&self.agent_apply_url(&spec.agent_name), &agent_manifest)
            .await?;
        let uid = agent_response
            .get("metadata")
            .and_then(|m| m.get("uid"))
            .and_then(|u| u.as_str())
            .ok_or_else(|| {
                KubeopencodeError::Parse("agent response missing metadata.uid".to_string())
            })?
            .to_string();
        let owner = AgentOwnerRef {
            name: spec.agent_name.clone(),
            uid,
        };

        // 2. ServiceAccount
        let sa_manifest =
            build_service_account_manifest(&spec.agent_name, &self.namespace, Some(&owner));
        self.apply_manifest(
            &self.service_account_apply_url(&spec.agent_name),
            &sa_manifest,
        )
        .await?;

        // 3. workspace ConfigMap (AGENTS.md と skills)
        let configmap_name = format!("{}-workspace", spec.agent_name);
        let cm_manifest = build_workspace_configmap_manifest(spec, &self.namespace, Some(&owner));
        self.apply_manifest(&self.configmap_apply_url(&configmap_name), &cm_manifest)
            .await?;

        // 4. ExternalSecret
        let es_name = format!("{}-credentials", spec.agent_name);
        let es_manifest = build_external_secret_manifest(
            &spec.agent_name,
            &self.strategy_agent,
            &self.namespace,
            Some(&owner),
        );
        self.apply_manifest(&self.external_secret_apply_url(&es_name), &es_manifest)
            .await?;

        Ok(())
    }

    async fn delete_strategy_agent(&self, agent_name: &str) -> Result<(), KubeopencodeError> {
        let response = self
            .http
            .delete(self.agent_item_url(agent_name))
            .send()
            .await
            .map_err(|e| KubeopencodeError::Network(e.to_string()))?;
        let status = response.status();
        if status.is_success() || status == StatusCode::NOT_FOUND {
            return Ok(());
        }
        let text = response.text().await.unwrap_or_default();
        Err(api_error(status, text))
    }
}

fn api_error(status: StatusCode, body: String) -> KubeopencodeError {
    let message = serde_json::from_str::<K8sStatusResponse>(&body)
        .ok()
        .and_then(|s| s.message.or(s.reason))
        .unwrap_or(body);
    KubeopencodeError::Api {
        status: status.as_u16(),
        message,
    }
}

pub(crate) fn build_task_cr_manifest(spec: &TaskCrSpec, namespace: &str) -> serde_json::Value {
    json!({
        "apiVersion": format!("{TASK_CR_GROUP}/{TASK_CR_VERSION}"),
        "kind": "Task",
        "metadata": {
            "name": spec.name,
            "namespace": namespace,
        },
        "spec": {
            "agentRef": { "name": spec.agent_name },
            "description": spec.description,
        },
    })
}

/// 共有用 alias。`Arc<dyn KubeopencodeClient + Send + Sync>` を頻繁に書くのを避ける。
pub type SharedKubeopencodeClient = Arc<dyn KubeopencodeClient + Send + Sync>;

/// テスト向け: 受け取ったリクエストを記録して `Ok` または事前設定の応答を返すフェイク
#[cfg(test)]
#[derive(Default)]
pub struct FakeKubeopencodeClient {
    pub created: tokio::sync::Mutex<Vec<TaskCrSpec>>,
    pub statuses: tokio::sync::Mutex<std::collections::HashMap<String, TaskCrStatus>>,
    pub create_error: tokio::sync::Mutex<Option<KubeopencodeError>>,
    pub reconciled: tokio::sync::Mutex<Vec<StrategyAgentSpec>>,
    pub deleted_agents: tokio::sync::Mutex<Vec<String>>,
    pub reconcile_error: tokio::sync::Mutex<Option<KubeopencodeError>>,
    pub delete_agent_error: tokio::sync::Mutex<Option<KubeopencodeError>>,
}

#[cfg(test)]
impl FakeKubeopencodeClient {
    pub fn new() -> Self {
        Self::default()
    }
    pub async fn set_status(&self, name: &str, status: TaskCrStatus) {
        self.statuses.lock().await.insert(name.to_string(), status);
    }
    pub async fn set_create_error(&self, err: KubeopencodeError) {
        *self.create_error.lock().await = Some(err);
    }
    pub async fn set_reconcile_error(&self, err: KubeopencodeError) {
        *self.reconcile_error.lock().await = Some(err);
    }
}

#[cfg(test)]
#[async_trait]
impl KubeopencodeClient for FakeKubeopencodeClient {
    async fn create_task(&self, spec: &TaskCrSpec) -> Result<(), KubeopencodeError> {
        if let Some(err) = self.create_error.lock().await.take() {
            return Err(err);
        }
        self.created.lock().await.push(spec.clone());
        Ok(())
    }
    async fn get_task_status(&self, name: &str) -> Result<TaskCrStatus, KubeopencodeError> {
        self.statuses
            .lock()
            .await
            .get(name)
            .cloned()
            .ok_or_else(|| KubeopencodeError::NotFound(name.to_string()))
    }
    async fn reconcile_strategy_agent(
        &self,
        spec: &StrategyAgentSpec,
    ) -> Result<(), KubeopencodeError> {
        if let Some(err) = self.reconcile_error.lock().await.take() {
            return Err(err);
        }
        self.reconciled.lock().await.push(spec.clone());
        Ok(())
    }
    async fn delete_strategy_agent(&self, agent_name: &str) -> Result<(), KubeopencodeError> {
        if let Some(err) = self.delete_agent_error.lock().await.take() {
            return Err(err);
        }
        self.deleted_agents
            .lock()
            .await
            .push(agent_name.to_string());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;
    use serde_json::json;
    use wiremock::matchers::{body_json, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn env_get<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
        move |key: &str| {
            pairs
                .iter()
                .find(|(k, _)| *k == key)
                .map(|(_, v)| (*v).to_string())
        }
    }

    fn expect_configured(source: KubeopencodeConfigSource) -> KubeopencodeConfig {
        match source {
            KubeopencodeConfigSource::Configured(c) => c,
            KubeopencodeConfigSource::Disabled => panic!("expected Configured, got Disabled"),
        }
    }

    #[rstest]
    #[case::missing_env(&[])]
    #[case::empty_env(&[("KUBEOPENCODE_API_URL", "")])]
    fn from_env_invalid_api_url_fails_fast(#[case] env: &[(&str, &str)]) {
        let result = KubeopencodeConfig::from_env_with(env_get(env));
        assert!(matches!(result, Err(KubeopencodeConfigError::Missing)));
    }

    #[rstest]
    fn from_env_disabled_sentinel_returns_disabled() {
        let result = KubeopencodeConfig::from_env_with(env_get(&[(
            "KUBEOPENCODE_API_URL",
            KUBEOPENCODE_DISABLED_SENTINEL,
        )]))
        .expect("disabled sentinel is accepted");
        assert!(matches!(result, KubeopencodeConfigSource::Disabled));
    }

    #[rstest]
    fn from_env_missing_strategy_mcp_url_fails_fast() {
        let result = KubeopencodeConfig::from_env_with(env_get(&[(
            "KUBEOPENCODE_API_URL",
            "https://kube.example/api",
        )]));
        assert!(matches!(
            result,
            Err(KubeopencodeConfigError::MissingStrategyMcpUrl)
        ));
    }

    #[rstest]
    fn from_env_with_url_returns_configured() {
        let config = expect_configured(
            KubeopencodeConfig::from_env_with(env_get(&[
                ("KUBEOPENCODE_API_URL", "https://kube.example/api"),
                ("KUBEOPENCODE_NAMESPACE", "custom-ns"),
                ("KUBEOPENCODE_TOKEN", "tok"),
                ("KUBEOPENCODE_INSECURE_TLS", "true"),
                ("STRATEGY_MCP_URL", "http://backend/mcp/strategy"),
                ("STRATEGY_AGENT_MODEL", "m-x"),
            ]))
            .expect("configured"),
        );
        assert_eq!(config.api_base_url, "https://kube.example/api");
        assert_eq!(config.namespace, "custom-ns");
        assert_eq!(config.bearer_token.as_deref(), Some("tok"));
        assert_eq!(config.ca_cert_path, None);
        assert!(config.insecure_tls);
        assert_eq!(
            config.strategy_agent,
            StrategyAgentSettings {
                mcp_url: "http://backend/mcp/strategy".into(),
                ssm_parameter_template: DEFAULT_SSM_PARAMETER_TEMPLATE.into(),
                model: "m-x".into(),
                small_model: DEFAULT_AGENT_SMALL_MODEL.into(),
            },
        );
    }

    #[rstest]
    fn from_env_defaults_namespace_when_unset() {
        let config = expect_configured(
            KubeopencodeConfig::from_env_with(env_get(&[
                ("KUBEOPENCODE_API_URL", "https://kube.example/api"),
                ("STRATEGY_MCP_URL", "http://backend/mcp/strategy"),
            ]))
            .expect("configured"),
        );
        assert_eq!(config.namespace, DEFAULT_NAMESPACE);
        assert!(!config.insecure_tls);
        assert_eq!(config.bearer_token, None);
    }

    #[rstest]
    #[case::pending("Pending", Some(TaskPhase::Pending))]
    #[case::running("Running", Some(TaskPhase::Running))]
    #[case::completed("Completed", Some(TaskPhase::Completed))]
    #[case::failed("Failed", Some(TaskPhase::Failed))]
    #[case::unknown("Mystery", None)]
    #[case::lower("running", Some(TaskPhase::Running))]
    fn parses_task_phase(#[case] raw: &str, #[case] expected: Option<TaskPhase>) {
        assert_eq!(TaskPhase::from_raw(raw), expected);
    }

    #[rstest]
    fn builds_manifest() {
        let spec = TaskCrSpec {
            name: "t-rader-abcd-xyz".into(),
            agent_name: "strategy-deadbeef".into(),
            description: "do the thing".into(),
        };
        assert_eq!(
            build_task_cr_manifest(&spec, "kubeopencode"),
            json!({
                "apiVersion": "kubeopencode.io/v1alpha1",
                "kind": "Task",
                "metadata": {
                    "name": "t-rader-abcd-xyz",
                    "namespace": "kubeopencode",
                },
                "spec": {
                    "agentRef": { "name": "strategy-deadbeef" },
                    "description": "do the thing",
                },
            }),
        );
    }

    fn http_client(server: &MockServer) -> HttpKubeopencodeClient {
        HttpKubeopencodeClient::new(KubeopencodeConfig {
            api_base_url: server.uri(),
            namespace: "kubeopencode".into(),
            bearer_token: Some("test-token".into()),
            ca_cert_path: None,
            insecure_tls: true,
            strategy_agent: StrategyAgentSettings {
                mcp_url: "http://backend/mcp/strategy".into(),
                ssm_parameter_template: DEFAULT_SSM_PARAMETER_TEMPLATE.into(),
                model: DEFAULT_AGENT_MODEL.into(),
                small_model: DEFAULT_AGENT_SMALL_MODEL.into(),
            },
        })
        .expect("build client")
    }

    #[tokio::test]
    async fn create_task_posts_manifest() {
        let server = MockServer::start().await;
        let spec = TaskCrSpec {
            name: "t-rader-abcd-xyz".into(),
            agent_name: "strategy-deadbeef".into(),
            description: "hello".into(),
        };
        let expected = build_task_cr_manifest(&spec, "kubeopencode");

        Mock::given(method("POST"))
            .and(path(
                "/apis/kubeopencode.io/v1alpha1/namespaces/kubeopencode/tasks",
            ))
            .and(body_json(&expected))
            .respond_with(ResponseTemplate::new(201).set_body_json(json!({
                "apiVersion": "kubeopencode.io/v1alpha1",
                "kind": "Task",
                "metadata": { "name": spec.name, "namespace": "kubeopencode" },
            })))
            .expect(1)
            .mount(&server)
            .await;

        let client = http_client(&server);
        let result = client.create_task(&spec).await;
        assert_eq!(result.map_err(|e| e.to_string()), Ok(()));
    }

    #[tokio::test]
    async fn create_task_already_exists_maps_to_conflict() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(409).set_body_json(json!({
                "kind": "Status",
                "message": "already exists",
                "reason": "AlreadyExists",
            })))
            .mount(&server)
            .await;
        let client = http_client(&server);
        let spec = TaskCrSpec {
            name: "dup".into(),
            agent_name: "strategy-1".into(),
            description: "x".into(),
        };
        let err = client.create_task(&spec).await.expect_err("expected error");
        assert_eq!(err.to_string(), "task already exists: dup");
    }

    #[tokio::test]
    async fn get_task_status_returns_phase_and_message() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path(
                "/apis/kubeopencode.io/v1alpha1/namespaces/kubeopencode/tasks/t-rader-1",
            ))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "status": { "phase": "Running", "message": "ok" }
            })))
            .mount(&server)
            .await;
        let client = http_client(&server);
        assert_eq!(
            client.get_task_status("t-rader-1").await.expect("ok"),
            TaskCrStatus {
                phase: Some(TaskPhase::Running),
                message: Some("ok".into()),
            },
        );
    }

    #[tokio::test]
    async fn reconcile_strategy_agent_applies_four_resources() {
        use wiremock::matchers::query_param;
        let server = MockServer::start().await;
        let strategy_id = uuid::Uuid::parse_str("12345678-1234-5678-1234-567812345678").unwrap();
        let agent_name = "strategy-12345678123456781234567812345678";

        // Agent CR の SSA → uid を返す
        Mock::given(method("PATCH"))
            .and(path(format!(
                "/apis/kubeopencode.io/v1alpha1/namespaces/kubeopencode/agents/{agent_name}"
            )))
            .and(query_param("fieldManager", FIELD_MANAGER))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "apiVersion": "kubeopencode.io/v1alpha1",
                "kind": "Agent",
                "metadata": { "name": agent_name, "namespace": "kubeopencode", "uid": "uid-1" },
            })))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("PATCH"))
            .and(path(format!(
                "/api/v1/namespaces/kubeopencode/serviceaccounts/{agent_name}"
            )))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("PATCH"))
            .and(path(format!(
                "/api/v1/namespaces/kubeopencode/configmaps/{agent_name}-workspace"
            )))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("PATCH"))
            .and(path(format!(
                "/apis/external-secrets.io/v1/namespaces/kubeopencode/externalsecrets/{agent_name}-credentials"
            )))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
            .expect(1)
            .mount(&server)
            .await;

        let client = http_client(&server);
        let spec = StrategyAgentSpec::new(strategy_id, "body", Default::default());
        let result = client.reconcile_strategy_agent(&spec).await;
        assert_eq!(result.map_err(|e| e.to_string()), Ok(()));
    }

    #[tokio::test]
    async fn reconcile_is_idempotent_on_repeated_calls() {
        let server = MockServer::start().await;
        let strategy_id = uuid::Uuid::new_v4();
        // 4 resources × 2 calls = 8 PATCHes must reach the server
        Mock::given(method("PATCH"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "metadata": { "uid": "uid-42" },
            })))
            .expect(8)
            .mount(&server)
            .await;
        let client = http_client(&server);
        let spec = StrategyAgentSpec::new(strategy_id, "body", Default::default());
        client.reconcile_strategy_agent(&spec).await.expect("first");
        client
            .reconcile_strategy_agent(&spec)
            .await
            .expect("second");
    }

    #[tokio::test]
    async fn reconcile_fails_when_agent_response_has_no_uid() {
        let server = MockServer::start().await;
        Mock::given(method("PATCH"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "metadata": { "name": "x" },
            })))
            .mount(&server)
            .await;
        let client = http_client(&server);
        let spec = StrategyAgentSpec::new(uuid::Uuid::new_v4(), "", Default::default());
        let err = client
            .reconcile_strategy_agent(&spec)
            .await
            .expect_err("expected parse error");
        assert!(matches!(err, KubeopencodeError::Parse(_)));
    }

    #[tokio::test]
    async fn delete_strategy_agent_treats_not_found_as_success() {
        let server = MockServer::start().await;
        Mock::given(method("DELETE"))
            .and(path(
                "/apis/kubeopencode.io/v1alpha1/namespaces/kubeopencode/agents/strategy-x",
            ))
            .respond_with(ResponseTemplate::new(404))
            .expect(1)
            .mount(&server)
            .await;
        let client = http_client(&server);
        client
            .delete_strategy_agent("strategy-x")
            .await
            .expect("not found should be Ok");
    }

    #[tokio::test]
    async fn delete_strategy_agent_propagates_other_errors() {
        let server = MockServer::start().await;
        Mock::given(method("DELETE"))
            .respond_with(ResponseTemplate::new(500).set_body_json(json!({
                "kind": "Status", "message": "boom",
            })))
            .mount(&server)
            .await;
        let client = http_client(&server);
        let err = client
            .delete_strategy_agent("strategy-x")
            .await
            .expect_err("err");
        assert!(matches!(err, KubeopencodeError::Api { status: 500, .. }));
    }

    #[tokio::test]
    async fn get_task_status_not_found() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;
        let client = http_client(&server);
        let err = client
            .get_task_status("missing")
            .await
            .expect_err("expected error");
        assert_eq!(err.to_string(), "task not found: missing");
    }
}
