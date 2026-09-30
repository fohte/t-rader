use core_application::daily_bar_source::DailyBarSourceError;
use std::time::Duration;

/// IBKR Client Portal Web API の呼び出しで発生するエラー
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum IbkrError {
    /// 指定された銘柄が見つからない
    #[error("instrument not found: {0}")]
    NotFound(String),

    /// ネットワーク通信エラー (接続失敗、タイムアウト等)
    #[error("network error: {0}")]
    Network(String),

    /// API がエラーレスポンスを返した (400, 403 等)
    #[error("api error (status {status}): {message}")]
    Api { status: u16, message: String },

    /// IBKR API が 429 を返した
    #[error("rate limited after {retries} retries")]
    RateLimited { retries: u32 },

    /// 共有 rate limit の取得に失敗
    #[error("rate limit error: {0}")]
    RateLimit(String),

    /// 共有 rate limit の待機上限に到達
    #[error("rate limit wait exceeded after {max_wait:?}")]
    RateLimitWaitExceeded { max_wait: Duration },

    /// レスポンスのパースに失敗
    #[error("failed to parse response: {0}")]
    Parse(String),
}

impl From<IbkrError> for DailyBarSourceError {
    fn from(error: IbkrError) -> Self {
        match error {
            IbkrError::NotFound(message) => Self::NotFound(message),
            error @ (IbkrError::RateLimited { .. } | IbkrError::RateLimitWaitExceeded { .. }) => {
                Self::RateLimited(error.to_string())
            }
            error => Self::Failed(error.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use rstest::rstest;

    use super::{DailyBarSourceError, IbkrError};

    #[rstest]
    #[case::not_found(
        IbkrError::NotFound("sample instrument".to_string()),
        DailyBarSourceError::NotFound("sample instrument".to_string()),
    )]
    #[case::rate_limited(
            IbkrError::RateLimited { retries: 3 },
            DailyBarSourceError::RateLimited("rate limited after 3 retries".to_string()),
        )]
    #[case::rate_limit_wait_exceeded(
            IbkrError::RateLimitWaitExceeded { max_wait: Duration::from_secs(30) },
            DailyBarSourceError::RateLimited("rate limit wait exceeded after 30s".to_string()),
        )]
    #[case::rate_limit_error(
            IbkrError::RateLimit("Redis operation failed".to_string()),
            DailyBarSourceError::Failed("rate limit error: Redis operation failed".to_string()),
        )]
    #[case::api(
        IbkrError::Api { status: 503, message: "source unavailable".to_string() },
        DailyBarSourceError::Failed("api error (status 503): source unavailable".to_string()),
    )]
    #[case::network(
        IbkrError::Network("connection failed".to_string()),
        DailyBarSourceError::Failed("network error: connection failed".to_string()),
    )]
    #[case::parse(
        IbkrError::Parse("malformed response".to_string()),
        DailyBarSourceError::Failed("failed to parse response: malformed response".to_string()),
    )]
    fn converts_to_daily_bar_source_error(
        #[case] error: IbkrError,
        #[case] expected: DailyBarSourceError,
    ) {
        assert_eq!(DailyBarSourceError::from(error), expected);
    }
}
