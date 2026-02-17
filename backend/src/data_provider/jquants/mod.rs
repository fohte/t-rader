#[cfg(test)]
mod mock;
mod response;
#[cfg(test)]
mod tests;

use chrono::{NaiveDate, TimeZone, Utc};
use reqwest::Url;
use rust_decimal::Decimal;

use crate::data_provider::{DataProvider, DataProviderError, DateRange};
use crate::models::bar::{Bar, Timeframe};
use crate::models::instrument::{Instrument, Market};
use response::{DailyBarsResponse, EquitiesMasterResponse, ErrorResponse};

const DEFAULT_BASE_URL: &str = "https://api.jquants.com/v2";
const MAX_RETRIES: u32 = 3;
const INITIAL_BACKOFF_MS: u64 = 500;
/// API サーバーのバグで同じ pagination_key が返り続けた場合の安全策
const MAX_PAGES: u32 = 100;

/// J-Quants API V2 クライアント
///
/// API Key 認証方式で J-Quants API V2 にアクセスする。
/// 429 (Rate Limited) と 5xx に対して指数バックオフでリトライする。
///
/// Debug は意図的に derive しない (api_key の漏洩防止)
pub struct JQuantsClient {
    http: reqwest::Client,
    base_url: String,
    api_key: String,
}

impl JQuantsClient {
    pub fn new(api_key: String) -> Result<Self, DataProviderError> {
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| DataProviderError::Network(e.to_string()))?;

        Ok(Self {
            http,
            base_url: DEFAULT_BASE_URL.to_string(),
            api_key,
        })
    }

    /// テスト用: ベース URL を差し替え可能にする
    #[cfg(test)]
    pub fn with_base_url(base_url: &str, api_key: &str) -> Result<Self, DataProviderError> {
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .map_err(|e| DataProviderError::Network(e.to_string()))?;

        Ok(Self {
            http,
            base_url: base_url.to_string(),
            api_key: api_key.to_string(),
        })
    }

    /// 指数バックオフ付き GET リクエスト
    ///
    /// 429 と 5xx に対してリトライする。それ以外のエラーは即座に返す。
    async fn get_with_retry(&self, url: &Url) -> Result<reqwest::Response, DataProviderError> {
        let mut last_error = None;
        let url_str = url.as_str();

        for attempt in 0..=MAX_RETRIES {
            if attempt > 0 {
                let backoff =
                    std::time::Duration::from_millis(INITIAL_BACKOFF_MS * 2u64.pow(attempt - 1));
                tracing::warn!(
                    attempt,
                    backoff_ms = backoff.as_millis() as u64,
                    url = url_str,
                    "リトライ待機中"
                );
                tokio::time::sleep(backoff).await;
            }

            let response = self
                .http
                .get(url.clone())
                .header("x-api-key", &self.api_key)
                .send()
                .await
                .map_err(|e| DataProviderError::Network(e.to_string()))?;

            let status = response.status().as_u16();

            match status {
                200..=299 => return Ok(response),
                429 => {
                    tracing::warn!(attempt, url = url_str, "レートリミット超過 (429)");
                    last_error = Some(DataProviderError::RateLimited { retries: attempt });
                }
                500..=599 => {
                    let message = Self::extract_error_message(response).await;
                    tracing::warn!(attempt, status, url = url_str, %message, "サーバーエラー、リトライ実行");
                    last_error = Some(DataProviderError::Api { status, message });
                }
                _ => {
                    let message = Self::extract_error_message(response).await;
                    return Err(DataProviderError::Api { status, message });
                }
            }
        }

        Err(last_error.unwrap_or(DataProviderError::RateLimited {
            retries: MAX_RETRIES,
        }))
    }

    /// レスポンスボディからエラーメッセージを抽出する
    async fn extract_error_message(response: reqwest::Response) -> String {
        let status = response.status().as_u16();
        response
            .json::<ErrorResponse>()
            .await
            .map(|e| e.message)
            .unwrap_or_else(|_| format!("request failed ({status})"))
    }

    /// f64 を Decimal に変換する
    fn to_decimal(value: f64) -> Result<Decimal, DataProviderError> {
        Decimal::try_from(value)
            .map_err(|e| DataProviderError::Parse(format!("invalid decimal value {value}: {e}")))
    }

    /// ベース URL にパスとクエリパラメータを付与して Url を構築する
    fn build_url(&self, path: &str, params: &[(&str, &str)]) -> Result<Url, DataProviderError> {
        let mut url = Url::parse(&format!("{}{path}", self.base_url))
            .map_err(|e| DataProviderError::Parse(format!("invalid base URL: {e}")))?;
        {
            let mut query = url.query_pairs_mut();
            for (key, value) in params {
                query.append_pair(key, value);
            }
        }
        Ok(url)
    }
}

impl DataProvider for JQuantsClient {
    async fn fetch_daily_bars(
        &self,
        instrument_id: &str,
        range: &DateRange,
    ) -> Result<Vec<Bar>, DataProviderError> {
        let mut all_bars = Vec::new();
        let mut pagination_key: Option<String> = None;
        let from_str = range.from.format("%Y%m%d").to_string();
        let to_str = range.to.format("%Y%m%d").to_string();

        for page in 0..MAX_PAGES {
            let mut params = vec![
                ("code", instrument_id),
                ("from", &from_str),
                ("to", &to_str),
            ];

            if let Some(key) = &pagination_key {
                params.push(("pagination_key", key));
            }

            let url = self.build_url("/equities/bars/daily", &params)?;

            tracing::debug!(%url, instrument_id, "J-Quants API から日足データを取得中");

            let response = self.get_with_retry(&url).await?;
            let body: DailyBarsResponse = response
                .json()
                .await
                .map_err(|e| DataProviderError::Parse(e.to_string()))?;

            for d in body.data {
                // 調整後価格が null のレコードはスキップ (非取引日等)
                let (Some(adj_open), Some(adj_high), Some(adj_low), Some(adj_close)) =
                    (d.adj_open, d.adj_high, d.adj_low, d.adj_close)
                else {
                    continue;
                };

                let date = NaiveDate::parse_from_str(&d.date, "%Y-%m-%d").map_err(|e| {
                    DataProviderError::Parse(format!("invalid date '{}': {e}", d.date))
                })?;

                let timestamp = Utc.from_utc_datetime(
                    &date
                        .and_hms_opt(0, 0, 0)
                        .ok_or_else(|| DataProviderError::Parse("invalid time".to_string()))?,
                );

                all_bars.push(Bar {
                    instrument_id: d.code,
                    timeframe: Timeframe::Daily,
                    timestamp,
                    open: Self::to_decimal(adj_open)?,
                    high: Self::to_decimal(adj_high)?,
                    low: Self::to_decimal(adj_low)?,
                    close: Self::to_decimal(adj_close)?,
                    volume: d.adj_volume.map(|v| v.round() as i64).unwrap_or(0),
                });
            }

            pagination_key = body.pagination_key;
            if pagination_key.is_none() {
                break;
            }

            if page == MAX_PAGES - 1 {
                tracing::warn!(
                    instrument_id,
                    max_pages = MAX_PAGES,
                    "ページネーション上限に到達、取得を打ち切り"
                );
            }
        }

        all_bars.sort_by_key(|b| b.timestamp);
        Ok(all_bars)
    }

    async fn fetch_instrument(&self, instrument_id: &str) -> Result<Instrument, DataProviderError> {
        let url = self.build_url("/equities/master", &[("code", instrument_id)])?;

        tracing::debug!(%url, instrument_id, "J-Quants API から銘柄情報を取得中");

        let response = self.get_with_retry(&url).await?;
        let body: EquitiesMasterResponse = response
            .json()
            .await
            .map_err(|e| DataProviderError::Parse(e.to_string()))?;

        let master = body.data.into_iter().next().ok_or_else(|| {
            DataProviderError::NotFound(format!("instrument '{instrument_id}' not found"))
        })?;

        Ok(Instrument {
            id: master.code,
            name: master.company_name,
            // J-Quants は東証上場銘柄のみを提供する
            market: Market::Tse,
            sector: master.sector_name,
        })
    }
}
