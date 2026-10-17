use std::sync::Arc;

use core_application::bars::SharedUsStockBarSource;
use core_application::us_stock_master_source::SharedUsStockMasterSource;
use gateway_alpaca::AlpacaClient;
use gateway_jquants::JQuantsPlan;
use gateway_sec::SecClient;
use sea_orm::DbErr;

mod eoddata_source;
mod news_and_central_bank_sources;

pub(super) use eoddata_source::eoddata_kospi_source_from_env;
pub(super) use news_and_central_bank_sources::{
    NewsAndCentralBankSources, initialize_news_and_central_bank_sources,
};

const CRON_TRIGGER_TIMER_ENABLED_ENV: &str = "CRON_TRIGGER_TIMER_ENABLED";

#[derive(Debug, thiserror::Error)]
pub(super) enum StartupError {
    #[error("internal error: {0}")]
    Internal(String),

    #[error("configuration error: {0}")]
    Config(String),

    // runtime failure も既存の process error prefix を維持する。
    #[error("configuration error: {0}")]
    Runtime(String),
}

impl From<DbErr> for StartupError {
    fn from(error: DbErr) -> Self {
        Self::Internal(error.to_string())
    }
}

fn parse_jquants_plan(value: Option<String>) -> Result<JQuantsPlan, StartupError> {
    let value = value.filter(|value| !value.is_empty()).ok_or_else(|| {
        StartupError::Config(
            "JQUANTS_PLAN is required when JQUANTS_API_KEY is configured".to_string(),
        )
    })?;
    value.parse().map_err(|error| {
        StartupError::Config(format!("invalid JQUANTS_PLAN value '{value}': {error}"))
    })
}

fn jquants_config(
    api_key: Option<String>,
    plan: Option<String>,
) -> Result<Option<(String, JQuantsPlan)>, StartupError> {
    match api_key.filter(|api_key| !api_key.is_empty()) {
        Some(api_key) => Ok(Some((api_key, parse_jquants_plan(plan)?))),
        None => Ok(None),
    }
}

pub(super) fn jquants_config_from_env() -> Result<Option<(String, JQuantsPlan)>, StartupError> {
    jquants_config(
        std::env::var("JQUANTS_API_KEY").ok(),
        std::env::var("JQUANTS_PLAN").ok(),
    )
}

fn alpaca_us_stock_bar_source(
    for_job: bool,
    redis_url: &str,
    api_key_id: Option<String>,
    secret_key: Option<String>,
) -> Result<Option<SharedUsStockBarSource>, StartupError> {
    if !for_job {
        return Ok(None);
    }

    match (api_key_id, secret_key) {
        (Some(api_key_id), Some(secret_key))
            if !api_key_id.trim().is_empty() && !secret_key.trim().is_empty() =>
        {
            let client = AlpacaClient::new(redis_url, api_key_id.trim(), secret_key.trim())
                .map_err(|error| {
                    StartupError::Config(format!("failed to initialize Alpaca client: {error}"))
                })?;
            tracing::info!("Alpaca US stock bar source initialized");
            Ok(Some(Arc::new(client)))
        }
        _ => {
            tracing::warn!(
                "ALPACA_API_KEY_ID または ALPACA_API_SECRET_KEY が未設定のため、この実行では米国株の株価を取り込みません"
            );
            Ok(None)
        }
    }
}

pub(super) fn alpaca_us_stock_bar_source_from_env(
    for_job: bool,
    redis_url: &str,
) -> Result<Option<SharedUsStockBarSource>, StartupError> {
    alpaca_us_stock_bar_source(
        for_job,
        redis_url,
        std::env::var("ALPACA_API_KEY_ID").ok(),
        std::env::var("ALPACA_API_SECRET_KEY").ok(),
    )
}

pub(super) fn us_stock_master_source(
    for_job: bool,
    user_agent: Option<String>,
) -> Result<Option<SharedUsStockMasterSource>, StartupError> {
    if !for_job {
        return Ok(None);
    }

    match user_agent {
        Some(user_agent) if !user_agent.trim().is_empty() => {
            let client = SecClient::new(&user_agent).map_err(|error| {
                StartupError::Config(format!("failed to initialize SEC client: {error}"))
            })?;
            tracing::info!("SEC US stock master source initialized");
            Ok(Some(Arc::new(client)))
        }
        _ => {
            tracing::warn!(
                "SEC_USER_AGENT が未設定のため、この実行では米国株のマスタを同期しません"
            );
            Ok(None)
        }
    }
}

pub(super) fn required_redis_url(value: Option<String>) -> Result<String, StartupError> {
    value.filter(|value| !value.is_empty()).ok_or_else(|| {
        StartupError::Config("REDIS_URL environment variable is not set".to_string())
    })
}

pub(super) fn cron_trigger_timer_enabled_from_env() -> Result<bool, StartupError> {
    match std::env::var(CRON_TRIGGER_TIMER_ENABLED_ENV) {
        Ok(value) => parse_cron_trigger_timer_enabled(Some(value)),
        Err(std::env::VarError::NotPresent) => parse_cron_trigger_timer_enabled(None),
        Err(error) => Err(StartupError::Config(format!(
            "failed to read {CRON_TRIGGER_TIMER_ENABLED_ENV}: {error}"
        ))),
    }
}

fn parse_cron_trigger_timer_enabled(value: Option<String>) -> Result<bool, StartupError> {
    value
        .map(|value| {
            value.parse().map_err(|error| {
                StartupError::Config(format!(
                    "invalid {CRON_TRIGGER_TIMER_ENABLED_ENV} value '{value}': {error}"
                ))
            })
        })
        .transpose()
        .map(|enabled| enabled.unwrap_or(false))
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case::api_key_missing(None, Some("invalid".to_string()), Ok(None))]
    #[case::api_key_empty(Some(String::new()), Some("invalid".to_string()), Ok(None))]
    #[case::plan_missing(
        Some("test-api-key".to_string()),
        None,
        Err("configuration error: JQUANTS_PLAN is required when JQUANTS_API_KEY is configured".to_string()),
    )]
    #[case::plan_empty(
        Some("test-api-key".to_string()),
        Some(String::new()),
        Err("configuration error: JQUANTS_PLAN is required when JQUANTS_API_KEY is configured".to_string()),
    )]
    #[case::plan_invalid(
        Some("test-api-key".to_string()),
        Some("enterprise".to_string()),
        Err("configuration error: invalid JQUANTS_PLAN value 'enterprise': unknown variant `enterprise`, expected one of `free`, `light`, `standard`, `premium`".to_string()),
    )]
    #[case::plan_valid(
        Some("test-api-key".to_string()),
        Some("standard".to_string()),
        Ok(Some(("test-api-key".to_string(), JQuantsPlan::Standard))),
    )]
    fn test_jquants_config(
        #[case] api_key: Option<String>,
        #[case] plan: Option<String>,
        #[case] expected: Result<Option<(String, JQuantsPlan)>, String>,
    ) {
        assert_eq!(
            jquants_config(api_key, plan).map_err(|error| error.to_string()),
            expected,
        );
    }

    #[rstest]
    #[case::api_mode(false, Some("synthetic-key"), Some("synthetic-secret"), false)]
    #[case::missing_key_id(true, None, Some("synthetic-secret"), false)]
    #[case::missing_secret(true, Some("synthetic-key"), None, false)]
    #[case::both_credentials(true, Some("synthetic-key"), Some("synthetic-secret"), true)]
    fn alpaca_source_requires_both_credentials_for_one_shot_jobs(
        #[case] for_job: bool,
        #[case] key: Option<&str>,
        #[case] secret: Option<&str>,
        #[case] expected: bool,
    ) {
        let actual = alpaca_us_stock_bar_source(
            for_job,
            "redis://localhost:6379",
            key.map(str::to_owned),
            secret.map(str::to_owned),
        )
        .map(|source| source.is_some())
        .map_err(|error| error.to_string());

        assert_eq!(actual, Ok(expected));
    }

    #[rstest]
    #[case::missing(
        None,
        Err("configuration error: REDIS_URL environment variable is not set")
    )]
    #[case::empty(
        Some(String::new()),
        Err("configuration error: REDIS_URL environment variable is not set")
    )]
    #[case::configured(
        Some("redis://localhost/".to_string()),
        Ok("redis://localhost/"),
    )]
    fn test_required_redis_url(
        #[case] value: Option<String>,
        #[case] expected: Result<&str, &str>,
    ) {
        assert_eq!(
            required_redis_url(value).map_err(|error| error.to_string()),
            expected.map(str::to_owned).map_err(str::to_owned),
        );
    }

    #[rstest]
    #[case::unset(None, Ok(false))]
    #[case::enabled(Some("true".to_string()), Ok(true))]
    #[case::disabled(Some("false".to_string()), Ok(false))]
    #[case::invalid(
        Some("yes".to_string()),
        Err("configuration error: invalid CRON_TRIGGER_TIMER_ENABLED value 'yes': provided string was not `true` or `false`".to_string()),
    )]
    fn parses_cron_trigger_timer_setting(
        #[case] value: Option<String>,
        #[case] expected: Result<bool, String>,
    ) {
        let actual = parse_cron_trigger_timer_enabled(value).map_err(|error| error.to_string());

        assert_eq!(actual, expected);
    }

    #[test]
    fn database_error_keeps_internal_error_display() {
        assert_eq!(
            StartupError::from(DbErr::Custom("database unavailable".into())).to_string(),
            "internal error: Custom Error: database unavailable",
        );
    }

    #[test]
    fn runtime_error_keeps_existing_configuration_error_display() {
        assert_eq!(
            StartupError::Runtime("server error: service exited".into()).to_string(),
            "configuration error: server error: service exited",
        );
    }
}
