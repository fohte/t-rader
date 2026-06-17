use std::time::Duration;

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
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
