use gateway_jquants::JQuantsPlan;
use sea_orm::DbErr;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};

mod eoddata_source;
mod news_and_central_bank_sources;

pub(super) use eoddata_source::eoddata_kospi_source_from_env;
pub(super) use news_and_central_bank_sources::{
    NewsAndCentralBankSources, initialize_news_and_central_bank_sources,
};

const DEFAULT_WORKER_ADMIN_UI_PORT: u16 = 3001;
const WORKER_ADMIN_UI_PORT_ENV: &str = "GRAPHILE_WORKER_ADMIN_UI_PORT";
const WORKER_ADMIN_UI_AUTH_HEADER_ENV: &str = "GRAPHILE_WORKER_ADMIN_UI_AUTH_HEADER_NAME";
const WORKER_ADMIN_UI_AUTH_VALUE_ENV: &str = "GRAPHILE_WORKER_ADMIN_UI_AUTH_HEADER_VALUE";

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

#[derive(Debug, PartialEq, Eq)]
pub(super) enum WorkerAdminUiAuth {
    None,
    Header { name: String, value: String },
}

pub(super) struct WorkerAdminUiSettings {
    pub(super) listen_addr: SocketAddr,
    pub(super) auth: WorkerAdminUiAuth,
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

fn configured_admin_ui_auth(
    header_name: Option<String>,
    header_value: Option<String>,
) -> Result<(IpAddr, WorkerAdminUiAuth), StartupError> {
    let header_name = header_name.filter(|value| !value.trim().is_empty());
    let header_value = header_value.filter(|value| !value.trim().is_empty());

    match (header_name, header_value) {
        (None, None) => Ok((IpAddr::V4(Ipv4Addr::LOCALHOST), WorkerAdminUiAuth::None)),
        (Some(name), Some(value)) => Ok((
            IpAddr::V4(Ipv4Addr::UNSPECIFIED),
            WorkerAdminUiAuth::Header { name, value },
        )),
        _ => Err(StartupError::Config(format!(
            "{WORKER_ADMIN_UI_AUTH_HEADER_ENV} and {WORKER_ADMIN_UI_AUTH_VALUE_ENV} must be set together"
        ))),
    }
}

fn worker_admin_ui_settings(
    port: Option<String>,
    auth_header_name: Option<String>,
    auth_header_value: Option<String>,
) -> Result<WorkerAdminUiSettings, StartupError> {
    let port = match port {
        Some(port) => port.parse::<u16>().map_err(|error| {
            StartupError::Config(format!(
                "invalid {WORKER_ADMIN_UI_PORT_ENV} value '{port}': {error}"
            ))
        })?,
        None => DEFAULT_WORKER_ADMIN_UI_PORT,
    };
    if port == 0 {
        return Err(StartupError::Config(format!(
            "{WORKER_ADMIN_UI_PORT_ENV} must be greater than zero"
        )));
    }

    let (listen_ip, auth) = configured_admin_ui_auth(auth_header_name, auth_header_value)?;
    Ok(WorkerAdminUiSettings {
        listen_addr: SocketAddr::new(listen_ip, port),
        auth,
    })
}

pub(super) fn worker_admin_ui_settings_from_env() -> Result<WorkerAdminUiSettings, StartupError> {
    worker_admin_ui_settings(
        std::env::var(WORKER_ADMIN_UI_PORT_ENV).ok(),
        std::env::var(WORKER_ADMIN_UI_AUTH_HEADER_ENV).ok(),
        std::env::var(WORKER_ADMIN_UI_AUTH_VALUE_ENV).ok(),
    )
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
    #[case::auth_unset(
        None,
        None,
        Ok((SocketAddr::from(([127, 0, 0, 1], 3001)), WorkerAdminUiAuth::None)),
    )]
    #[case::auth_blank(
        Some(String::new()),
        Some(String::new()),
        Ok((SocketAddr::from(([127, 0, 0, 1], 3001)), WorkerAdminUiAuth::None)),
    )]
    #[case::header_auth_configured(
        Some("x-proxy-identity".to_string()),
        Some("authorized-user".to_string()),
        Ok((
            SocketAddr::from(([0, 0, 0, 0], 3001)),
            WorkerAdminUiAuth::Header {
                name: "x-proxy-identity".to_string(),
                value: "authorized-user".to_string(),
            },
        )),
    )]
    #[case::header_name_only(
        Some("x-proxy-identity".to_string()),
        None,
        Err("configuration error: GRAPHILE_WORKER_ADMIN_UI_AUTH_HEADER_NAME and GRAPHILE_WORKER_ADMIN_UI_AUTH_HEADER_VALUE must be set together".to_string()),
    )]
    #[case::header_value_only(
        None,
        Some("authorized-user".to_string()),
        Err("configuration error: GRAPHILE_WORKER_ADMIN_UI_AUTH_HEADER_NAME and GRAPHILE_WORKER_ADMIN_UI_AUTH_HEADER_VALUE must be set together".to_string()),
    )]
    fn test_worker_admin_ui_auth_settings(
        #[case] auth_header_name: Option<String>,
        #[case] auth_header_value: Option<String>,
        #[case] expected: Result<(SocketAddr, WorkerAdminUiAuth), String>,
    ) {
        let actual = worker_admin_ui_settings(None, auth_header_name, auth_header_value)
            .map(|settings| (settings.listen_addr, settings.auth))
            .map_err(|error| error.to_string());

        assert_eq!(actual, expected);
    }

    #[rstest]
    #[case::default_port(None, Ok(SocketAddr::from(([127, 0, 0, 1], 3001))))]
    #[case::custom_port(Some("4321".to_string()), Ok(SocketAddr::from(([127, 0, 0, 1], 4321))))]
    #[case::invalid_port(
        Some("invalid".to_string()),
        Err("configuration error: invalid GRAPHILE_WORKER_ADMIN_UI_PORT value 'invalid': invalid digit found in string".to_string()),
    )]
    #[case::zero_port(
        Some("0".to_string()),
        Err("configuration error: GRAPHILE_WORKER_ADMIN_UI_PORT must be greater than zero".to_string()),
    )]
    fn test_worker_admin_ui_port(
        #[case] port: Option<String>,
        #[case] expected: Result<SocketAddr, String>,
    ) {
        let actual = worker_admin_ui_settings(port, None, None)
            .map(|settings| settings.listen_addr)
            .map_err(|error| error.to_string());

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
