#[cfg(test)]
pub(crate) mod mock;
mod response;
#[cfg(test)]
mod tests;

use chrono::{DateTime, TimeZone, Utc};
use reqwest::Url;
use rust_decimal::Decimal;

use crate::data_provider::{DataProvider, DataProviderError, DateRange};
use crate::models::bar::{Bar, Timeframe};
use crate::models::instrument::{Instrument, Market};
use response::{ErrorResponse, HistoryResponse, StocksResponse};

/// Client Portal Gateway のデフォルト URL
///
/// 本番では VKE 内に Client Portal Gateway を常駐させ、その URL を IBKR_BASE_URL で上書きする。
const DEFAULT_BASE_URL: &str = "https://localhost:5000/v1/api";

/// 日本株のデフォルト取引所コード (Tokyo Stock Exchange, JPY)
const DEFAULT_EXCHANGE: &str = "TSEJ";

const MAX_RETRIES: u32 = 3;
const INITIAL_BACKOFF_MS: u64 = 500;

/// IBKR Client Portal Web API クライアント
///
/// CP Gateway の HTTP API (`/v1/api/`) を呼び出す。認証は Gateway 側の Web ログインで維持されるため、
/// 本クライアントは Gateway へ HTTP リクエストを送るだけでよい。Bearer トークンなど追加の
/// 認証ヘッダが必要な場合は `session_token` で差し込める。
///
/// 株価データの取得は次の 2 段階で行う:
/// 1. `trsrv/stocks` で銘柄コードから IBKR の conid を解決する
/// 2. `iserver/marketdata/history` で conid に対する日足を取得する
///
/// Debug は意図的に derive しない (session_token の漏洩防止)
pub struct IbkrClient {
    http: reqwest::Client,
    base_url: String,
    session_token: Option<String>,
    exchange: String,
}

impl IbkrClient {
    pub fn new(
        base_url: Option<String>,
        session_token: Option<String>,
        exchange: Option<String>,
    ) -> Result<Self, DataProviderError> {
        let http = reqwest::Client::builder()
            // CP Gateway は自己署名証明書を使うことが多いため、運用上は信頼できる接続経路 (cluster 内) で
            // のみ利用する前提とする。証明書検証を緩める設定はここでは入れない。
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| DataProviderError::Network(e.to_string()))?;

        let mut base_url = base_url
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_BASE_URL.to_string());
        // build_url の format!("{}{path}", ...) で二重スラッシュにならないよう末尾を落とす
        if base_url.ends_with('/') {
            base_url.pop();
        }
        Url::parse(&base_url)
            .map_err(|e| DataProviderError::Parse(format!("invalid IBKR_BASE_URL: {e}")))?;

        let exchange = exchange
            .filter(|s| !s.trim().is_empty())
            .map(|s| s.to_uppercase())
            .unwrap_or_else(|| DEFAULT_EXCHANGE.to_string());

        Ok(Self {
            http,
            base_url,
            session_token,
            exchange,
        })
    }

    /// テスト用: 任意の base URL を指定して構築する
    #[cfg(test)]
    pub fn with_base_url(base_url: &str) -> Result<Self, DataProviderError> {
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .map_err(|e| DataProviderError::Network(e.to_string()))?;

        Ok(Self {
            http,
            base_url: base_url.to_string(),
            session_token: None,
            exchange: DEFAULT_EXCHANGE.to_string(),
        })
    }

    /// 指数バックオフ付き GET。429 と 5xx に対してのみリトライする。
    async fn get_with_retry(&self, url: &Url) -> Result<reqwest::Response, DataProviderError> {
        let mut last_error: Option<DataProviderError> = None;
        let url_str = url.as_str();

        for attempt in 0..=MAX_RETRIES {
            if attempt > 0 {
                let backoff =
                    std::time::Duration::from_millis(INITIAL_BACKOFF_MS * 2u64.pow(attempt - 1));
                tracing::warn!(
                    attempt,
                    backoff_ms = backoff.as_millis() as u64,
                    url = url_str,
                    "IBKR API リトライ待機中"
                );
                tokio::time::sleep(backoff).await;
            }

            let mut req = self.http.get(url.clone());
            if let Some(token) = &self.session_token {
                req = req.bearer_auth(token);
            }

            let response = req
                .send()
                .await
                .map_err(|e| DataProviderError::Network(e.to_string()))?;

            let status = response.status().as_u16();
            match status {
                200..=299 => return Ok(response),
                429 => {
                    tracing::warn!(attempt, url = url_str, "IBKR レートリミット超過 (429)");
                    last_error = Some(DataProviderError::RateLimited { retries: attempt });
                }
                500..=599 => {
                    let message = Self::extract_error_message(response).await;
                    tracing::warn!(attempt, status, url = url_str, %message, "IBKR サーバーエラー、リトライ実行");
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

    async fn extract_error_message(response: reqwest::Response) -> String {
        let status = response.status().as_u16();
        response
            .json::<ErrorResponse>()
            .await
            .map(ErrorResponse::into_message)
            .unwrap_or_else(|_| format!("request failed ({status})"))
    }

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

    fn to_decimal(value: f64) -> Result<Decimal, DataProviderError> {
        Decimal::try_from(value)
            .map_err(|e| DataProviderError::Parse(format!("invalid decimal value {value}: {e}")))
    }

    /// 銘柄コードと取引所から conid と銘柄情報を 1 回の API 呼び出しで解決する。
    async fn lookup_stock(
        &self,
        instrument_id: &str,
    ) -> Result<(i64, Instrument), DataProviderError> {
        let url = self.build_url("/trsrv/stocks", &[("symbols", instrument_id)])?;

        tracing::debug!(%url, instrument_id, "IBKR API から銘柄情報を取得中");

        let response = self.get_with_retry(&url).await?;
        let body: StocksResponse = response
            .json()
            .await
            .map_err(|e| DataProviderError::Parse(e.to_string()))?;

        let issuers = body.get(instrument_id).ok_or_else(|| {
            DataProviderError::NotFound(format!("instrument '{instrument_id}' not found"))
        })?;

        // 同名 issuer が複数返ることがあるため、指定の取引所と一致する contract を持つ最初の issuer を使う
        for issuer in issuers {
            if let Some(contract) = issuer
                .contracts
                .iter()
                .find(|c| c.exchange == self.exchange)
            {
                let name = issuer
                    .name
                    .clone()
                    .unwrap_or_else(|| instrument_id.to_string());
                return Ok((
                    contract.conid,
                    Instrument {
                        id: instrument_id.to_string(),
                        name,
                        market: Market::Tse,
                        sector: None,
                    },
                ));
            }
        }

        Err(DataProviderError::NotFound(format!(
            "instrument '{instrument_id}' not listed on {}",
            self.exchange
        )))
    }
}

/// `DateRange` から IBKR の `period` パラメータ (例: `30d`) と `startTime` パラメータを導出する。
///
/// IBKR の `startTime` は「取得範囲の終端 (最新バーの時刻)」を意味し、そこから `period` 分過去に遡る。
/// `to` の翌日 00:00:00 UTC を終端とし、`to - from + 1` 日分を要求する。
fn period_and_start_time(range: &DateRange) -> (String, String) {
    let days = (range.to - range.from).num_days().max(0) + 1;
    let start_time = Utc
        .from_utc_datetime(
            &range
                .to
                .succ_opt()
                .unwrap_or(range.to)
                .and_hms_opt(0, 0, 0)
                .unwrap_or_default(),
        )
        .format("%Y%m%d-%H:%M:%S")
        .to_string();
    (format!("{days}d"), start_time)
}

impl DataProvider for IbkrClient {
    async fn fetch_daily_bars(
        &self,
        instrument_id: &str,
        range: &DateRange,
    ) -> Result<Vec<Bar>, DataProviderError> {
        let (conid, _) = self.lookup_stock(instrument_id).await?;

        let (period, start_time) = period_and_start_time(range);
        let conid_str = conid.to_string();
        let params = [
            ("conid", conid_str.as_str()),
            ("period", period.as_str()),
            ("bar", "1d"),
            ("outsideRth", "false"),
            ("startTime", start_time.as_str()),
        ];
        let url = self.build_url("/iserver/marketdata/history", &params)?;

        tracing::debug!(%url, instrument_id, conid, "IBKR API から日足データを取得中");

        let response = self.get_with_retry(&url).await?;
        let body: HistoryResponse = response
            .json()
            .await
            .map_err(|e| DataProviderError::Parse(e.to_string()))?;

        let from_dt: DateTime<Utc> = Utc.from_utc_datetime(
            &range
                .from
                .and_hms_opt(0, 0, 0)
                .ok_or_else(|| DataProviderError::Parse("invalid from date".to_string()))?,
        );
        // to は inclusive なので翌日 00:00 を排他的上限に使う
        let to_exclusive_date = range.to.succ_opt().unwrap_or(range.to);
        let to_dt: DateTime<Utc> = Utc.from_utc_datetime(
            &to_exclusive_date
                .and_hms_opt(0, 0, 0)
                .ok_or_else(|| DataProviderError::Parse("invalid to date".to_string()))?,
        );

        let mut bars = Vec::with_capacity(body.data.len());
        for h in body.data {
            let timestamp = Utc.timestamp_millis_opt(h.t).single().ok_or_else(|| {
                DataProviderError::Parse(format!("invalid timestamp millis: {}", h.t))
            })?;

            // 要求された期間外のバーは捨てる (IBKR は period から逆算して余分に返すことがある)
            if timestamp < from_dt || timestamp >= to_dt {
                continue;
            }

            // IBKR の `t` の規約 (TSE のときに UTC 基準か JST 基準か) は実 API で要検証。
            // 現状は受信した UTC タイムスタンプの日付に 00:00 を割り当てる単純正規化に留める。
            // 取引日とずれることが判明したら TZ 変換 (Asia/Tokyo) を入れる。
            let date = timestamp.date_naive();
            let normalized = Utc.from_utc_datetime(
                &date
                    .and_hms_opt(0, 0, 0)
                    .ok_or_else(|| DataProviderError::Parse("invalid time".to_string()))?,
            );

            bars.push(Bar {
                instrument_id: instrument_id.to_string(),
                timeframe: Timeframe::Daily,
                timestamp: normalized,
                open: Self::to_decimal(h.o)?,
                high: Self::to_decimal(h.h)?,
                low: Self::to_decimal(h.l)?,
                close: Self::to_decimal(h.c)?,
                volume: h.v.round() as i64,
            });
        }

        bars.sort_by_key(|b| b.timestamp);
        Ok(bars)
    }

    async fn fetch_instrument(&self, instrument_id: &str) -> Result<Instrument, DataProviderError> {
        let (_, instrument) = self.lookup_stock(instrument_id).await?;
        Ok(instrument)
    }
}

#[cfg(test)]
mod period_tests {
    use super::*;
    use chrono::NaiveDate;
    use rstest::rstest;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap_or_default()
    }

    #[rstest]
    #[case::same_day(d(2025, 1, 6), d(2025, 1, 6), "1d", "20250107-00:00:00")]
    #[case::three_days(d(2025, 1, 6), d(2025, 1, 8), "3d", "20250109-00:00:00")]
    #[case::month(d(2025, 1, 1), d(2025, 1, 31), "31d", "20250201-00:00:00")]
    fn test_period_and_start_time(
        #[case] from: NaiveDate,
        #[case] to: NaiveDate,
        #[case] expected_period: &str,
        #[case] expected_start_time: &str,
    ) {
        let range = DateRange { from, to };
        assert_eq!(
            period_and_start_time(&range),
            (expected_period.to_string(), expected_start_time.to_string())
        );
    }
}
