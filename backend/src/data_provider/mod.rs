pub mod ibkr;
pub mod jquants;
#[cfg(test)]
mod mock;
pub mod news;

pub use core_application::{DailyBarSource, DailyBarSourceError, DateRange, SharedDailyBarSource};

/// データプロバイダーで発生しうるエラー
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum DataProviderError {
    /// 指定された銘柄が見つからない
    #[error("instrument not found: {0}")]
    NotFound(String),

    /// ネットワーク通信エラー (接続失敗、タイムアウト等)
    #[error("network error: {0}")]
    Network(String),

    /// API がエラーレスポンスを返した (400, 403 等)
    #[error("api error (status {status}): {message}")]
    Api { status: u16, message: String },

    /// レートリミット超過でリトライ上限に到達
    #[error("rate limited after {retries} retries")]
    RateLimited { retries: u32 },

    /// クライアント側のレート制限ウィンドウが満杯
    #[error("rate limit window full (max {max_requests} requests)")]
    RateLimitWindowFull { max_requests: usize },

    /// レスポンスのパースに失敗
    #[error("failed to parse response: {0}")]
    Parse(String),

    /// データプロバイダー内部の DB アクセスに失敗 (集約結果の永続化など)
    #[error("database error: {0}")]
    Database(String),
}

impl From<DataProviderError> for DailyBarSourceError {
    fn from(error: DataProviderError) -> Self {
        match error {
            DataProviderError::NotFound(message) => Self::NotFound(message),
            DataProviderError::RateLimited { retries } => {
                Self::RateLimited(format!("rate limited after {retries} retries"))
            }
            DataProviderError::RateLimitWindowFull { max_requests } => Self::RateLimited(format!(
                "rate limit window full (max {max_requests} requests)"
            )),
            error => Self::Failed(error.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::{DailyBarSourceError, DataProviderError};

    #[rstest]
    #[case::not_found(
        DataProviderError::NotFound("sample instrument".to_string()),
        DailyBarSourceError::NotFound("sample instrument".to_string()),
    )]
    #[case::rate_limited(
        DataProviderError::RateLimited { retries: 3 },
        DailyBarSourceError::RateLimited("rate limited after 3 retries".to_string()),
    )]
    #[case::rate_limit_window_full(
        DataProviderError::RateLimitWindowFull { max_requests: 5 },
        DailyBarSourceError::RateLimited("rate limit window full (max 5 requests)".to_string()),
    )]
    #[case::api(
        DataProviderError::Api { status: 503, message: "source unavailable".to_string() },
        DailyBarSourceError::Failed("api error (status 503): source unavailable".to_string()),
    )]
    #[case::network(
        DataProviderError::Network("connection failed".to_string()),
        DailyBarSourceError::Failed("network error: connection failed".to_string()),
    )]
    #[case::parse(
        DataProviderError::Parse("malformed response".to_string()),
        DailyBarSourceError::Failed("failed to parse response: malformed response".to_string()),
    )]
    #[case::database(
        DataProviderError::Database("write failed".to_string()),
        DailyBarSourceError::Failed("database error: write failed".to_string()),
    )]
    fn converts_provider_errors(
        #[case] error: DataProviderError,
        #[case] expected: DailyBarSourceError,
    ) {
        assert_eq!(DailyBarSourceError::from(error), expected);
    }
}
