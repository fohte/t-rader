use std::str::FromStr;

use chrono::Utc;
use reqwest::Url;
use rust_decimal::Decimal;

use crate::data_provider::DataProviderError;
use crate::data_provider::macro_data::{MacroDataProvider, MacroTick};

const DEFAULT_BASE_URL: &str = "https://stooq.com/q/l/";
/// 取得タイムアウト
const HTTP_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

/// Stooq から取得するシンボル一覧
///
/// `(stooq_symbol, display_symbol)` のペア。display は frontend ヘッダ用の和名。
/// Stooq の遅延は最大 15min。
const SYMBOLS: &[(&str, &str)] = &[
    ("^nkx", "日経225"),
    ("^tpx", "TOPIX"),
    ("jpy", "USD/JPY"),
    ("^vix", "VIX"),
    ("es.f", "S&P500 fut"),
    ("10usy.b", "US10Y"),
];

/// Stooq CSV API を使ったマクロ指標 DataProvider
pub struct StooqClient {
    http: reqwest::Client,
    base_url: String,
}

impl StooqClient {
    pub fn new() -> Result<Self, DataProviderError> {
        Self::with_base_url(DEFAULT_BASE_URL)
    }

    pub fn with_base_url(base_url: &str) -> Result<Self, DataProviderError> {
        let http = reqwest::Client::builder()
            .timeout(HTTP_TIMEOUT)
            .build()
            .map_err(|e| DataProviderError::Network(e.to_string()))?;
        Ok(Self {
            http,
            base_url: base_url.to_string(),
        })
    }

    fn build_url(&self) -> Result<Url, DataProviderError> {
        let symbols = SYMBOLS
            .iter()
            .map(|(s, _)| *s)
            .collect::<Vec<_>>()
            .join(",");
        let mut url = Url::parse(&self.base_url)
            .map_err(|e| DataProviderError::Parse(format!("invalid base URL: {e}")))?;
        {
            let mut q = url.query_pairs_mut();
            q.append_pair("s", &symbols);
            // s = symbol, d2 = date (YYYY-MM-DD), t2 = time, c = close (current),
            // p2 = percent change vs previous close (前日比)
            q.append_pair("f", "sd2t2cp2");
            q.append_pair("h", "");
            q.append_pair("e", "csv");
        }
        Ok(url)
    }
}

#[async_trait::async_trait]
impl MacroDataProvider for StooqClient {
    async fn fetch_macro_ticks(&self) -> Result<Vec<MacroTick>, DataProviderError> {
        let url = self.build_url()?;
        let response = self
            .http
            .get(url)
            .send()
            .await
            .map_err(|e| DataProviderError::Network(e.to_string()))?;

        let status = response.status().as_u16();
        if !(200..300).contains(&status) {
            return Err(DataProviderError::Api {
                status,
                message: format!("stooq returned status {status}"),
            });
        }

        let body = response
            .text()
            .await
            .map_err(|e| DataProviderError::Network(e.to_string()))?;
        parse_csv(&body, Utc::now())
    }
}

/// Stooq CSV をパースして `MacroTick` 列を返す
///
/// 想定 CSV (`f=sd2t2cp2`):
///
/// ```text
/// Symbol,Date,Time,Close,Change
/// ^NKX,2026-06-25,15:00:00,38420.55,-0.62
/// ...
/// ```
///
/// `Change` は前日終値からの変化率 (%)。値が取得できないシンボル (`N/D` 等) はスキップする。
fn parse_csv(
    body: &str,
    fetched_at: chrono::DateTime<Utc>,
) -> Result<Vec<MacroTick>, DataProviderError> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .from_reader(body.as_bytes());

    let mut ticks = Vec::new();
    for record in reader.records() {
        let row = record.map_err(|e| DataProviderError::Parse(format!("csv: {e}")))?;
        let Some(stooq_symbol) = row.get(0) else {
            continue;
        };
        let Some(close_str) = row.get(3) else {
            continue;
        };
        let Some(change_str) = row.get(4) else {
            continue;
        };

        let Some((_, display)) = SYMBOLS
            .iter()
            .find(|(s, _)| s.eq_ignore_ascii_case(stooq_symbol))
        else {
            continue;
        };

        let Ok(close) = Decimal::from_str(close_str) else {
            continue;
        };
        let Ok(pct) = Decimal::from_str(change_str) else {
            continue;
        };

        ticks.push(MacroTick {
            symbol: (*display).to_string(),
            value: format_value(close),
            pct: pct.round_dp(2),
            fetched_at,
        });
    }

    // 全シンボルが N/D 等で skip された (= Stooq 側のデータ欠落) ケースは
    // 「成功 (空)」ではなく fetch 失敗として扱う。`record_success(vec![])` で
    // stale_since が消えてしまうと、handler が空配列のまま返してしまう。
    if ticks.is_empty() {
        return Err(DataProviderError::Parse(
            "no symbols parsed from stooq response".into(),
        ));
    }

    Ok(ticks)
}

/// Decimal を小数 2 桁の文字列に整形する (末尾の余分な 0 は残す)
fn format_value(value: Decimal) -> String {
    format!("{:.2}", value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use indoc::indoc;
    use rstest::rstest;

    fn fixed_now() -> chrono::DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 6, 25, 6, 0, 0)
            .single()
            .expect("valid time")
    }

    fn dec(s: &str) -> Decimal {
        Decimal::from_str(s).expect("valid decimal")
    }

    #[rstest]
    fn test_parse_csv_returns_ticks_for_known_symbols() {
        let csv = indoc! {"
            Symbol,Date,Time,Close,Change
            ^NKX,2026-06-25,15:00:00,38420.55,-0.62
            ^TPX,2026-06-25,15:00:00,2711.30,-0.41
            JPY,2026-06-25,15:00:00,157.84,0.38
            ^VIX,2026-06-25,15:00:00,18.92,4.71
            ES.F,2026-06-25,15:00:00,5284.00,-0.55
            10USY.B,2026-06-25,15:00:00,4.482,1.84
        "};

        assert_eq!(
            parse_csv(csv, fixed_now()).expect("parse ok"),
            vec![
                MacroTick {
                    symbol: "日経225".into(),
                    value: "38420.55".into(),
                    pct: dec("-0.62"),
                    fetched_at: fixed_now(),
                },
                MacroTick {
                    symbol: "TOPIX".into(),
                    value: "2711.30".into(),
                    pct: dec("-0.41"),
                    fetched_at: fixed_now(),
                },
                MacroTick {
                    symbol: "USD/JPY".into(),
                    value: "157.84".into(),
                    pct: dec("0.38"),
                    fetched_at: fixed_now(),
                },
                MacroTick {
                    symbol: "VIX".into(),
                    value: "18.92".into(),
                    pct: dec("4.71"),
                    fetched_at: fixed_now(),
                },
                MacroTick {
                    symbol: "S&P500 fut".into(),
                    value: "5284.00".into(),
                    pct: dec("-0.55"),
                    fetched_at: fixed_now(),
                },
                MacroTick {
                    symbol: "US10Y".into(),
                    value: "4.48".into(),
                    pct: dec("1.84"),
                    fetched_at: fixed_now(),
                },
            ],
        );
    }

    #[rstest]
    fn test_parse_csv_skips_unknown_and_invalid_rows() {
        let csv = indoc! {"
            Symbol,Date,Time,Close,Change
            ^NKX,2026-06-25,15:00:00,38420.0,1.11
            UNKNOWN,2026-06-25,15:00:00,1.0,0.0
            ^VIX,N/D,N/D,N/D,N/D
        "};

        assert_eq!(
            parse_csv(csv, fixed_now()).expect("parse ok"),
            vec![MacroTick {
                symbol: "日経225".into(),
                value: "38420.00".into(),
                pct: dec("1.11"),
                fetched_at: fixed_now(),
            }],
        );
    }
}
