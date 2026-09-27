use std::str::FromStr;

use rust_decimal::Decimal;
use serde::Deserialize;

use super::Paginated;

/// J-Quants API V2 バリュエーション指標レスポンス (`GET /v2/equities/valuation`)
#[derive(Debug, Deserialize)]
pub(crate) struct ValuationResponse {
    pub data: Vec<ValuationRecord>,
    pub pagination_key: Option<String>,
}

impl Paginated for ValuationResponse {
    type Item = ValuationRecord;

    fn into_parts(self) -> (Vec<ValuationRecord>, Option<String>) {
        (self.data, self.pagination_key)
    }
}

/// J-Quants API V2 バリュエーション指標 1 レコード。
#[derive(Debug, Deserialize)]
pub(crate) struct ValuationRecord {
    #[serde(rename = "Date")]
    pub date: String,
    #[serde(rename = "Code")]
    pub code: String,
    #[serde(
        rename = "EPS",
        default,
        deserialize_with = "deserialize_optional_decimal"
    )]
    pub eps: Option<Decimal>,
    #[serde(
        rename = "FwdEPS",
        default,
        deserialize_with = "deserialize_optional_decimal"
    )]
    pub fwd_eps: Option<Decimal>,
    #[serde(
        rename = "BPS",
        default,
        deserialize_with = "deserialize_optional_decimal"
    )]
    pub bps: Option<Decimal>,
    #[serde(
        rename = "ROE",
        default,
        deserialize_with = "deserialize_optional_decimal"
    )]
    pub roe: Option<Decimal>,
    #[serde(
        rename = "FwdROE",
        default,
        deserialize_with = "deserialize_optional_decimal"
    )]
    pub fwd_roe: Option<Decimal>,
    #[serde(
        rename = "PER",
        default,
        deserialize_with = "deserialize_optional_decimal"
    )]
    pub per: Option<Decimal>,
    #[serde(
        rename = "FwdPER",
        default,
        deserialize_with = "deserialize_optional_decimal"
    )]
    pub fwd_per: Option<Decimal>,
    #[serde(
        rename = "PBR",
        default,
        deserialize_with = "deserialize_optional_decimal"
    )]
    pub pbr: Option<Decimal>,
    #[serde(
        rename = "MktCap",
        default,
        deserialize_with = "deserialize_optional_decimal"
    )]
    pub mkt_cap: Option<Decimal>,
}

fn deserialize_optional_decimal<'de, D>(deserializer: D) -> Result<Option<Decimal>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    match Option::<serde_json::Value>::deserialize(deserializer)? {
        None | Some(serde_json::Value::Null) => Ok(None),
        Some(serde_json::Value::Number(number)) => Decimal::from_str(&number.to_string())
            .map(Some)
            .map_err(serde::de::Error::custom),
        Some(serde_json::Value::String(value)) if value.is_empty() => Ok(None),
        Some(other) => Err(serde::de::Error::custom(format!(
            "expected a number or empty string, got {other:?}"
        ))),
    }
}
