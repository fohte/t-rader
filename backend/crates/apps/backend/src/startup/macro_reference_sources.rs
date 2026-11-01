use std::sync::Arc;

use backend::services::use_cases::UseCases;
use core_application::{
    calendar::source::SharedCalendarEventSource, macro_reference::MacroReferenceUseCases,
    macro_reference_source::SharedMacroReferenceSource,
};
use gateway_bea::BeaMacroReferenceSource;
use gateway_bls::BlsMacroReferenceSource;
use gateway_boj::BojMacroReferenceSource;
use gateway_census::CensusMacroReferenceSource;
use gateway_customs::CustomsMacroReferenceSource;
use gateway_e_stat::{EStatCalendarEventSource, EStatMacroReferenceSource};
use gateway_ecb::EcbMacroReferenceSource;
use gateway_esri::EsriMacroReferenceSource;
use gateway_fed::FedMacroReferenceSource;
use gateway_meti::MetiMacroReferenceSource;

use super::StartupError;

pub(super) fn macro_reference_sources_from_redis(
    redis_url: &str,
) -> Result<Vec<SharedMacroReferenceSource>, StartupError> {
    Ok(vec![
        Arc::new(BojMacroReferenceSource::new(redis_url).map_err(|error| {
            StartupError::Config(format!(
                "failed to initialize BOJ reference source: {error}"
            ))
        })?),
        Arc::new(EcbMacroReferenceSource::new(redis_url).map_err(|error| {
            StartupError::Config(format!(
                "failed to initialize ECB reference source: {error}"
            ))
        })?),
        Arc::new(FedMacroReferenceSource::new(redis_url).map_err(|error| {
            StartupError::Config(format!(
                "failed to initialize Federal Reserve reference source: {error}"
            ))
        })?),
        Arc::new(EStatMacroReferenceSource::new(redis_url).map_err(|error| {
            StartupError::Config(format!(
                "failed to initialize e-Stat reference source: {error}"
            ))
        })?),
        Arc::new(
            CustomsMacroReferenceSource::new(redis_url).map_err(|error| {
                StartupError::Config(format!(
                    "failed to initialize Customs reference source: {error}"
                ))
            })?,
        ),
        Arc::new(EsriMacroReferenceSource::new(redis_url).map_err(|error| {
            StartupError::Config(format!(
                "failed to initialize ESRI reference source: {error}"
            ))
        })?),
        Arc::new(MetiMacroReferenceSource::new(redis_url).map_err(|error| {
            StartupError::Config(format!(
                "failed to initialize METI reference source: {error}"
            ))
        })?),
        Arc::new(BlsMacroReferenceSource::new(redis_url).map_err(|error| {
            StartupError::Config(format!(
                "failed to initialize BLS reference source: {error}"
            ))
        })?),
        Arc::new(BeaMacroReferenceSource::new(redis_url).map_err(|error| {
            StartupError::Config(format!(
                "failed to initialize BEA reference source: {error}"
            ))
        })?),
        Arc::new(CensusMacroReferenceSource::new(redis_url).map_err(|error| {
            StartupError::Config(format!(
                "failed to initialize Census reference source: {error}"
            ))
        })?),
    ])
}

pub(crate) fn initialize_macro_reference_pipeline(
    redis_url: &str,
    use_cases: &UseCases,
) -> Result<(SharedCalendarEventSource, MacroReferenceUseCases), StartupError> {
    let e_stat_calendar_source: SharedCalendarEventSource =
        Arc::new(EStatCalendarEventSource::new(redis_url).map_err(|error| {
            StartupError::Config(format!(
                "failed to initialize e-Stat calendar source: {error}"
            ))
        })?);
    tracing::info!("e-Stat calendar source initialized");
    let macro_references =
        use_cases.macro_references(macro_reference_sources_from_redis(redis_url)?);
    Ok((e_stat_calendar_source, macro_references))
}
