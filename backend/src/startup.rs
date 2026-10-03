use gateway_jquants::JQuantsPlan;
use sea_orm::DbErr;

#[derive(Debug, thiserror::Error)]
pub(super) enum StartupError {
    #[error("internal error: {0}")]
    Internal(String),

    #[error("configuration error: {0}")]
    Config(String),
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

pub(super) fn required_redis_url(value: Option<String>) -> Result<String, StartupError> {
    value.filter(|value| !value.is_empty()).ok_or_else(|| {
        StartupError::Config("REDIS_URL environment variable is not set".to_string())
    })
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case::api_key_missing(None, Some("invalid".to_string()), Ok(None))]
    #[case::api_key_empty(Some(String::new()), Some("invalid".to_string()), Ok(None))]
    #[case::plan_missing(Some("test-api-key".to_string()), None, Err(()))]
    #[case::plan_empty(Some("test-api-key".to_string()), Some(String::new()), Err(()))]
    #[case::plan_invalid(
        Some("test-api-key".to_string()),
        Some("enterprise".to_string()),
        Err(()),
    )]
    #[case::plan_valid(
        Some("test-api-key".to_string()),
        Some("standard".to_string()),
        Ok(Some(("test-api-key".to_string(), JQuantsPlan::Standard))),
    )]
    fn test_jquants_config(
        #[case] api_key: Option<String>,
        #[case] plan: Option<String>,
        #[case] expected: Result<Option<(String, JQuantsPlan)>, ()>,
    ) {
        assert_eq!(jquants_config(api_key, plan).map_err(|_| ()), expected);
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

    #[test]
    fn database_error_keeps_internal_error_display() {
        assert_eq!(
            StartupError::from(DbErr::Custom("database unavailable".into())).to_string(),
            "internal error: Custom Error: database unavailable",
        );
    }
}
