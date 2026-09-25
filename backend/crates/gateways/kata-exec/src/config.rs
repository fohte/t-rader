use std::time::Duration;

pub(crate) const DEFAULT_POLL_INTERVAL: Duration = Duration::from_millis(500);
pub(crate) const DEFAULT_WALL_CLOCK_TIMEOUT: Duration = Duration::from_secs(60);
pub(crate) const DEFAULT_MAX_OUTPUT_BYTES: usize = 1024 * 1024;
pub(crate) const DEFAULT_CPU_LIMIT: &str = "500m";
pub(crate) const DEFAULT_MEMORY_LIMIT: &str = "256Mi";
pub(crate) const DEFAULT_EPHEMERAL_STORAGE_LIMIT: &str = "64Mi";
pub(crate) const DEFAULT_NAMESPACE: &str = "t-rader-exec";
pub(crate) const DEFAULT_IMAGE: &str = "ghcr.io/fohte/t-rader/python-exec:latest";

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
