use core_domain::equity_master::{EquityMasterAttributeValue, EquityMasterEntry};
use thiserror::Error;

pub const TSE_SECTOR33_DERIVE_FROM: &str = "tse_sector33";

pub type EquityMasterGroupValueExtractor =
    for<'a> fn(&'a EquityMasterEntry) -> Option<&'a EquityMasterAttributeValue>;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum EquityMasterGroupAttributeError {
    #[error("unsupported derive_from value: {0}")]
    UnsupportedDeriveFrom(String),
}

pub fn group_value_extractor(
    derive_from: &str,
) -> Result<EquityMasterGroupValueExtractor, EquityMasterGroupAttributeError> {
    match derive_from {
        TSE_SECTOR33_DERIVE_FROM => Ok(|entry| entry.tse_sector33.as_ref()),
        unsupported => Err(EquityMasterGroupAttributeError::UnsupportedDeriveFrom(
            unsupported.to_owned(),
        )),
    }
}
