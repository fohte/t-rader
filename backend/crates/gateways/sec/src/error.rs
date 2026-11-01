#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum SecClientError {
    #[error("invalid SEC client configuration: {0}")]
    Config(String),
    #[error("SEC request failed: {0}")]
    Request(String),
    #[error("SEC returned HTTP status {0}")]
    Status(u16),
    #[error("invalid SEC response: {0}")]
    Parse(String),
    #[error("SEC shared rate limit failed: {0}")]
    RateLimit(String),
}
