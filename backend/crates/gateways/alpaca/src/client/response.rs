use std::str::FromStr;

use chrono::{DateTime, NaiveDate};
use core_application::bars::UsStockSplit;
use core_domain::bar::{Bar, Timeframe};
use rust_decimal::{Decimal, prelude::ToPrimitive};
use serde::Deserialize;

use super::AlpacaError;

#[derive(Deserialize)]
pub(super) struct BarsResponse {
    #[serde(default)]
    pub(super) bars: std::collections::HashMap<String, Vec<AlpacaBar>>,
    pub(super) next_page_token: Option<String>,
}

#[derive(Deserialize)]
pub(super) struct CorporateActionsResponse {
    #[serde(default)]
    pub(super) forward_splits: Vec<AlpacaSplit>,
    #[serde(default)]
    pub(super) reverse_splits: Vec<AlpacaSplit>,
    pub(super) next_page_token: Option<String>,
}

#[derive(Deserialize)]
pub(super) struct AlpacaSplit {
    pub(super) symbol: String,
    ex_date: String,
    old_rate: AlpacaDecimal,
    new_rate: AlpacaDecimal,
}

impl AlpacaSplit {
    pub(super) fn into_split(self, instrument_id: String) -> Result<UsStockSplit, AlpacaError> {
        Ok(UsStockSplit {
            instrument_id,
            ex_date: NaiveDate::parse_from_str(&self.ex_date, "%Y-%m-%d")
                .map_err(|error| AlpacaError::Parse(error.to_string()))?,
            old_rate: self.old_rate.into_decimal()?,
            new_rate: self.new_rate.into_decimal()?,
        })
    }
}

#[derive(Deserialize)]
#[serde(untagged)]
enum AlpacaDecimal {
    String(String),
    Number(serde_json::Number),
}

impl AlpacaDecimal {
    fn into_decimal(self) -> Result<Decimal, AlpacaError> {
        let value = match self {
            Self::String(value) => value,
            Self::Number(value) => value.to_string(),
        };
        Decimal::from_str(&value).map_err(|error| AlpacaError::Parse(error.to_string()))
    }
}

#[derive(Deserialize)]
pub(super) struct AlpacaBar {
    t: String,
    o: serde_json::Number,
    h: serde_json::Number,
    l: serde_json::Number,
    c: serde_json::Number,
    v: serde_json::Number,
}

impl AlpacaBar {
    pub(super) fn into_bar(
        self,
        instrument_id: &str,
        timeframe: Timeframe,
    ) -> Result<Bar, AlpacaError> {
        let timestamp = DateTime::parse_from_rfc3339(&self.t)
            .map_err(|error| AlpacaError::Parse(error.to_string()))?
            .to_utc();
        let volume = decimal_from_number(self.v)?;
        if volume.fract() != Decimal::ZERO {
            return Err(AlpacaError::Parse(format!(
                "bar volume must be an integer: {volume}"
            )));
        }

        Ok(Bar {
            instrument_id: instrument_id.to_owned(),
            timeframe,
            timestamp,
            open: decimal_from_number(self.o)?,
            high: decimal_from_number(self.h)?,
            low: decimal_from_number(self.l)?,
            close: decimal_from_number(self.c)?,
            volume: volume.to_i64().ok_or_else(|| {
                AlpacaError::Parse(format!("bar volume is out of range: {volume}"))
            })?,
            adjustment_factor: Decimal::ONE,
        })
    }
}

fn decimal_from_number(value: serde_json::Number) -> Result<Decimal, AlpacaError> {
    Decimal::from_str(&value.to_string()).map_err(|error| AlpacaError::Parse(error.to_string()))
}
