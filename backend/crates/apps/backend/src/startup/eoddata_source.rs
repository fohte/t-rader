use std::sync::Arc;

use core_application::indicator_observation_source::SharedIndicatorObservationSource;
use gateway_eoddata::EodDataClient;

use super::StartupError;

pub(crate) fn eoddata_kospi_source_from_env()
-> Result<Option<SharedIndicatorObservationSource>, StartupError> {
    match std::env::var("EODDATA_API_KEY") {
        Ok(api_key) if !api_key.trim().is_empty() => {
            let client = Arc::new(EodDataClient::new(api_key.trim().to_string()).map_err(
                |error| {
                    StartupError::Config(format!("failed to initialize EODData client: {error}"))
                },
            )?);
            let source: SharedIndicatorObservationSource = client;
            Ok(Some(source))
        }
        _ => {
            tracing::warn!("EODDATA_API_KEY が未設定のため、KOSPI の取り込みを起動しません");
            Ok(None)
        }
    }
}
