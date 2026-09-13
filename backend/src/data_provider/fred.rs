//! FRED (Federal Reserve Economic Data) API からマクロ指標の観測値を取得するクライアント。
//!
//! <https://fred.stlouisfed.org/docs/api/fred/series_observations.html>

use std::str::FromStr;

use chrono::NaiveDate;
use reqwest::Url;
use rust_decimal::Decimal;
use serde::Deserialize;

use crate::data_provider::DataProviderError;

const DEFAULT_BASE_URL: &str = "https://api.stlouisfed.org/fred/series/observations";
const HTTP_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

/// 欠損値を表す FRED の慣習的な表記
const MISSING_VALUE: &str = ".";

/// FRED から取得した 1 日分の観測値
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FredObservation {
    pub date: NaiveDate,
    pub value: Decimal,
}

#[derive(Debug, Deserialize)]
struct ObservationsResponse {
    observations: Vec<RawObservation>,
}

#[derive(Debug, Deserialize)]
struct RawObservation {
    date: String,
    value: String,
}

pub struct FredClient {
    http: reqwest::Client,
    base_url: String,
    api_key: String,
}

impl FredClient {
    pub fn new(api_key: String) -> Result<Self, DataProviderError> {
        Self::with_base_url(api_key, DEFAULT_BASE_URL)
    }

    pub fn with_base_url(api_key: String, base_url: &str) -> Result<Self, DataProviderError> {
        let http = reqwest::Client::builder()
            .timeout(HTTP_TIMEOUT)
            .build()
            .map_err(|e| DataProviderError::Network(e.to_string()))?;
        Ok(Self {
            http,
            base_url: base_url.to_string(),
            api_key,
        })
    }

    fn build_url(
        &self,
        series_id: &str,
        observation_start: Option<NaiveDate>,
    ) -> Result<Url, DataProviderError> {
        let mut url = Url::parse(&self.base_url)
            .map_err(|e| DataProviderError::Parse(format!("invalid base URL: {e}")))?;
        {
            let mut q = url.query_pairs_mut();
            q.append_pair("series_id", series_id);
            q.append_pair("api_key", &self.api_key);
            q.append_pair("file_type", "json");
            if let Some(start) = observation_start {
                q.append_pair("observation_start", &start.format("%Y-%m-%d").to_string());
            }
        }
        Ok(url)
    }

    /// 指定した系列の観測値を `observation_start` 以降取得する。`observation_start` が
    /// `None` の場合は FRED 側のデフォルト (系列の提供開始日) から取得する。
    /// 欠損値 (`"."`) は結果から除く。
    pub async fn fetch_observations(
        &self,
        series_id: &str,
        observation_start: Option<NaiveDate>,
    ) -> Result<Vec<FredObservation>, DataProviderError> {
        let url = self.build_url(series_id, observation_start)?;
        let response = self
            .http
            .get(url)
            .send()
            .await
            // URL に api_key を含むため、エラーメッセージに残らないよう取り除く
            .map_err(|e| DataProviderError::Network(e.without_url().to_string()))?;

        let status = response.status().as_u16();
        if !(200..300).contains(&status) {
            return Err(DataProviderError::Api {
                status,
                message: format!("FRED returned status {status} for series {series_id}"),
            });
        }

        let body = response
            .text()
            .await
            .map_err(|e| DataProviderError::Network(e.without_url().to_string()))?;
        parse_observations(&body)
    }
}

fn parse_observations(body: &str) -> Result<Vec<FredObservation>, DataProviderError> {
    let parsed: ObservationsResponse = serde_json::from_str(body)
        .map_err(|e| DataProviderError::Parse(format!("FRED response: {e}")))?;

    let mut observations = Vec::with_capacity(parsed.observations.len());
    for raw in parsed.observations {
        if raw.value == MISSING_VALUE {
            continue;
        }
        let date = NaiveDate::parse_from_str(&raw.date, "%Y-%m-%d")
            .map_err(|e| DataProviderError::Parse(format!("invalid date '{}': {e}", raw.date)))?;
        let value = Decimal::from_str(&raw.value)
            .map_err(|e| DataProviderError::Parse(format!("invalid value '{}': {e}", raw.value)))?;
        observations.push(FredObservation { date, value });
    }
    Ok(observations)
}

#[cfg(test)]
mod tests {
    use indoc::indoc;
    use rstest::rstest;

    use super::*;

    fn dec(s: &str) -> Decimal {
        Decimal::from_str(s).expect("valid decimal")
    }

    fn date(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).expect("valid date")
    }

    #[rstest]
    fn test_parse_observations_skips_missing_values() {
        let body = indoc! {r#"
            {
                "observations": [
                    {"realtime_start": "2026-09-13", "realtime_end": "2026-09-13", "date": "2026-09-01", "value": "147.50"},
                    {"realtime_start": "2026-09-13", "realtime_end": "2026-09-13", "date": "2026-09-02", "value": "."},
                    {"realtime_start": "2026-09-13", "realtime_end": "2026-09-13", "date": "2026-09-03", "value": "148.02"}
                ]
            }
        "#};

        assert_eq!(
            parse_observations(body).expect("parse ok"),
            vec![
                FredObservation {
                    date: date(2026, 9, 1),
                    value: dec("147.50"),
                },
                FredObservation {
                    date: date(2026, 9, 3),
                    value: dec("148.02"),
                },
            ],
        );
    }

    #[rstest]
    #[case::without_observation_start(
        None,
        "https://api.stlouisfed.org/fred/series/observations?series_id=DEXJPUS&api_key=test-key&file_type=json"
    )]
    #[case::with_observation_start(
        Some(date(2026, 1, 1)),
        "https://api.stlouisfed.org/fred/series/observations?series_id=DEXJPUS&api_key=test-key&file_type=json&observation_start=2026-01-01"
    )]
    fn test_build_url(#[case] observation_start: Option<NaiveDate>, #[case] expected: &str) {
        let client =
            FredClient::with_base_url("test-key".to_string(), DEFAULT_BASE_URL).expect("client");

        let url = client.build_url("DEXJPUS", observation_start).expect("url");

        assert_eq!(url.as_str(), expected);
    }
}
