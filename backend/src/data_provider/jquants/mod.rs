mod daily_bars;
mod earnings_schedule;
mod edinet_holdings;
mod equities_master;
mod fin_summary;
mod margin;
#[cfg(test)]
pub(crate) mod mock;
mod rate_limiter;
mod response;
mod short_selling;
#[cfg(test)]
mod tests;
mod valuation;

use chrono::{NaiveDate, TimeZone, Utc};
use reqwest::Url;
use rust_decimal::Decimal;

use rate_limiter::RateLimiter;

use crate::data_provider::{DataProviderError, DateRange};
use crate::models::bar::{Bar, Timeframe};
use crate::models::instrument::{Instrument, Market};
use crate::models::jquants_plan::JQuantsPlan;
use response::{
    EarningsDateResponse, EdinetDocumentsResponse, EquitiesMasterResponse, ErrorResponse,
    FinSummaryResponse, Paginated, ValuationResponse,
};

const DEFAULT_BASE_URL: &str = "https://api.jquants.com/v2";
const MAX_RETRIES: u32 = 3;
const INITIAL_BACKOFF_MS: u64 = 500;
/// API サーバーのバグで同じ pagination_key が返り続けた場合の安全策
const MAX_PAGES: u32 = 100;

/// `/fins/summary` (財務情報) 固有のレート制限 (契約プランと別枠、公式ページ記載の値)。
/// 大幅に超過すると 5 分程度アクセスが完全に遮断されるため、契約プラン上限より低い方を使う。
const FIN_SUMMARY_RATE_LIMIT_PER_MINUTE: usize = 60;

/// API 側の制限調整や複数インスタンス稼働に備え、契約プランの公称レートリミットの
/// 半分を実効上限とする安全係数。
const RATE_LIMIT_SAFETY_FACTOR: usize = 2;

/// `limit` に `RATE_LIMIT_SAFETY_FACTOR` を適用し、下限 1 でフロアする。
fn apply_safety_margin(limit: usize) -> usize {
    (limit / RATE_LIMIT_SAFETY_FACTOR).max(1)
}

/// 429 を受けてから、このクライアントの全呼び出しの送信を止める時間。
const RATE_LIMIT_COOLDOWN: std::time::Duration = std::time::Duration::from_secs(5 * 60);
/// 429 の cooldown 待ちを何回まで繰り返すか。これを超えてなお 429 が続く場合はエラーを返す。
const MAX_RATE_LIMIT_RETRIES: u32 = 3;

/// J-Quants API V2 クライアント
///
/// API Key 認証方式で J-Quants API V2 にアクセスする。
/// アプリケーションレベルのレートリミッターを内蔵する。5xx には指数バックオフで
/// リトライし、429 を受けた場合は全呼び出しの送信を `RATE_LIMIT_COOLDOWN` の間止めて
/// 待つ (cooldown)。レートリミッターの上限は契約プランに安全マージンを適用する。
///
/// Debug は意図的に derive しない (api_key の漏洩防止)
pub struct JQuantsClient {
    http: reqwest::Client,
    base_url: String,
    api_key: String,
    rate_limiter: RateLimiter,
    plan: JQuantsPlan,
}

impl JQuantsClient {
    pub fn new(api_key: String, plan: JQuantsPlan) -> Result<Self, DataProviderError> {
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| DataProviderError::Network(e.to_string()))?;

        Ok(Self {
            http,
            base_url: DEFAULT_BASE_URL.to_string(),
            api_key,
            rate_limiter: RateLimiter::new(RATE_LIMIT_COOLDOWN),
            plan,
        })
    }

    /// テスト用: ベース URL を差し替え可能にする
    #[cfg(test)]
    pub fn with_base_url(
        base_url: &str,
        api_key: &str,
        plan: JQuantsPlan,
    ) -> Result<Self, DataProviderError> {
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .map_err(|e| DataProviderError::Network(e.to_string()))?;

        Ok(Self {
            http,
            base_url: base_url.to_string(),
            api_key: api_key.to_string(),
            // 429 cooldown を短縮し、window 枠超過では待たずに失敗させる
            rate_limiter: RateLimiter::new_fail_fast(std::time::Duration::from_millis(50)),
            plan,
        })
    }

    /// 契約プランで取得できる範囲を返す。
    fn plan_date_range(&self, today: NaiveDate) -> DateRange {
        let (from, to) = self.plan.range(today);
        DateRange { from, to }
    }

    pub(crate) fn known_fetchable_date_range(&self, today: NaiveDate) -> DateRange {
        self.plan_date_range(today)
    }

    /// Standard 以上で利用できるデータの取得範囲を返す。
    pub(crate) fn standard_plan_date_range(
        &self,
        today: NaiveDate,
        data_name: &str,
    ) -> Option<DateRange> {
        match self.plan {
            JQuantsPlan::Standard | JQuantsPlan::Premium => Some(self.plan_date_range(today)),
            plan => {
                tracing::debug!(
                    ?plan,
                    data_name,
                    "Standard 以上の契約プランが必要なため取得できません"
                );
                None
            }
        }
    }

    /// レートリミッターの現在の上限 (1 分あたりのリクエスト数)
    ///
    /// 契約プランの公称値に `RATE_LIMIT_SAFETY_FACTOR` による安全マージンを適用した値を返す。
    fn current_rate_limit(&self) -> usize {
        apply_safety_margin(self.plan.rate_limit_per_minute())
    }

    /// 指数バックオフ付き GET リクエスト
    ///
    /// レートリミッターで送信間隔を制御した上で、429 と 5xx に対してリトライする。
    /// それ以外のエラーは即座に返す。`max_requests` はウィンドウ内の許容リクエスト数
    /// (通常は `current_rate_limit()`。エンドポイント固有の上限がある場合はそれとの min)。
    ///
    /// 429 はレートリミッターの cooldown 待ち (`RateLimiter::acquire` 内) に任せるため
    /// 指数バックオフは行わず、`MAX_RATE_LIMIT_RETRIES` 回まで cooldown 明けを待って
    /// 再試行する。5xx は従来通り `MAX_RETRIES` 回まで指数バックオフでリトライする。
    async fn get_with_retry(
        &self,
        url: &Url,
        max_requests: usize,
    ) -> Result<reqwest::Response, DataProviderError> {
        let url_str = url.as_str();
        let mut retry_attempt = 0u32;
        let mut rate_limit_attempt = 0u32;

        loop {
            self.rate_limiter.acquire(max_requests).await?;

            if retry_attempt > 0 {
                let backoff = std::time::Duration::from_millis(
                    INITIAL_BACKOFF_MS * 2u64.pow(retry_attempt - 1),
                );
                tracing::warn!(
                    attempt = retry_attempt,
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
                    rate_limit_attempt += 1;
                    tracing::warn!(
                        attempt = rate_limit_attempt,
                        url = url_str,
                        "レートリミット超過 (429)、送信を停止して待機します"
                    );
                    self.rate_limiter.note_rate_limited().await;

                    if rate_limit_attempt > MAX_RATE_LIMIT_RETRIES {
                        return Err(DataProviderError::RateLimited {
                            retries: MAX_RATE_LIMIT_RETRIES,
                        });
                    }
                }
                500..=599 => {
                    let message = Self::extract_error_message(response).await;
                    tracing::warn!(attempt = retry_attempt, status, url = url_str, %message, "サーバーエラー、リトライ実行");

                    retry_attempt += 1;
                    if retry_attempt > MAX_RETRIES {
                        return Err(DataProviderError::Api { status, message });
                    }
                }
                _ => {
                    let message = Self::extract_error_message(response).await;
                    return Err(DataProviderError::Api { status, message });
                }
            }
        }
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

    /// `path` に `params` を付けて `pagination_key` が尽きるまでページを追い、
    /// 各ページの要素を 1 つの配列に連結して返す。暴走防止に `MAX_PAGES` で打ち切る。
    async fn fetch_all_pages<R>(
        &self,
        path: &str,
        params: &[(&str, &str)],
        max_requests: usize,
    ) -> Result<Vec<R::Item>, DataProviderError>
    where
        R: serde::de::DeserializeOwned + Paginated,
    {
        let mut all_items = Vec::new();
        let mut pagination_key: Option<String> = None;

        for page in 0..MAX_PAGES {
            let mut page_params = params.to_vec();
            if let Some(key) = &pagination_key {
                page_params.push(("pagination_key", key));
            }

            let url = self.build_url(path, &page_params)?;

            tracing::debug!(%url, "J-Quants API からページを取得中");

            let response = self.get_with_retry(&url, max_requests).await?;
            let body: R = response
                .json()
                .await
                .map_err(|e| DataProviderError::Parse(e.to_string()))?;

            let (items, next_key) = body.into_parts();
            all_items.extend(items);

            pagination_key = next_key;
            if pagination_key.is_none() {
                break;
            }

            if page == MAX_PAGES - 1 {
                tracing::warn!(
                    %url,
                    max_pages = MAX_PAGES,
                    "ページネーション上限に到達、取得を打ち切り"
                );
            }
        }

        Ok(all_items)
    }

    /// EDINET 由来のデータ (大量保有報告書 / 政策保有株式 / 大株主状況) を `date` (提出日) 指定で取得する。
    /// `path` は `/edinet/large-volume-shareholders` 等。該当書類が無ければ空配列を返す。
    pub async fn fetch_edinet_documents(
        &self,
        path: &str,
        date: NaiveDate,
    ) -> Result<Vec<serde_json::Value>, DataProviderError> {
        let date_str = date.format("%Y%m%d").to_string();
        let params = [("date", date_str.as_str())];
        self.fetch_all_pages::<EdinetDocumentsResponse>(path, &params, self.current_rate_limit())
            .await
    }

    /// `/equities/valuation` を `date` 指定で取得する。全上場銘柄の指標が返る。
    pub(crate) async fn fetch_valuation_by_date(
        &self,
        date: NaiveDate,
    ) -> Result<Vec<response::ValuationRecord>, DataProviderError> {
        let date_str = date.format("%Y-%m-%d").to_string();
        let params = [("date", date_str.as_str())];
        self.fetch_all_pages::<ValuationResponse>(
            "/equities/valuation",
            &params,
            self.current_rate_limit(),
        )
        .await
    }

    /// `/fins/summary` を `date` (開示日) 指定で取得する。全上場銘柄のその日の開示分が
    /// まとめて返る。フィールド数が多く記載欄も可変 (IFRS 適用会社は経常利益が空欄等) のため、
    /// 個別フィールドへのパースはせず生の JSON のまま返す (呼び出し側で必要な値を取り出す)。
    pub(crate) async fn fetch_fin_summary_by_date(
        &self,
        date: NaiveDate,
    ) -> Result<Vec<serde_json::Value>, DataProviderError> {
        let date_str = date.format("%Y-%m-%d").to_string();
        let params = [("date", date_str.as_str())];
        let max_requests = self
            .current_rate_limit()
            .min(apply_safety_margin(FIN_SUMMARY_RATE_LIMIT_PER_MINUTE));

        self.fetch_all_pages::<FinSummaryResponse>("/fins/summary", &params, max_requests)
            .await
    }

    /// `/fins/earnings-date` を `date` (公表日) 指定で取得する。全上場銘柄のその日の公表分が
    /// まとめて返る。予定日の変更も新しい公表日の行として返るため、同じ (code, fq_name) の
    /// 過去の公表日の行は上書きされず変更履歴として蓄積される。
    pub(crate) async fn fetch_earnings_date_by_date(
        &self,
        date: NaiveDate,
    ) -> Result<Vec<response::EarningsDateRecord>, DataProviderError> {
        let date_str = date.format("%Y-%m-%d").to_string();
        let params = [("date", date_str.as_str())];
        self.fetch_all_pages::<EarningsDateResponse>(
            "/fins/earnings-date",
            &params,
            self.current_rate_limit(),
        )
        .await
    }
}

impl JQuantsClient {
    pub async fn fetch_instrument(
        &self,
        instrument_id: &str,
    ) -> Result<Instrument, DataProviderError> {
        let url = self.build_url("/equities/master", &[("code", instrument_id)])?;

        tracing::debug!(%url, instrument_id, "J-Quants API から銘柄情報を取得中");

        let response = self.get_with_retry(&url, self.current_rate_limit()).await?;
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
            product_category: master.product_category,
        })
    }
}

/// J-Quants の 5 桁ローカルコードをアプリ内の 4 桁銘柄コード規約に正規化する。
/// 末尾桁は銘柄種別 (普通株は "0") を表すため、"0" 終わりのときだけ 4 桁に短縮する。
/// それ以外 (優先株等、稀) は対応する 4 桁銘柄が無いため 5 桁のまま扱う。
fn normalize_local_code(code: &str) -> &str {
    if code.len() == 5 && code.ends_with('0') {
        &code[..4]
    } else {
        code
    }
}

/// `DailyBar` 1 件を `Bar` に変換する。調整後価格が null (非取引日等) のレコードは
/// `None` を返す。
fn parse_daily_bar(
    d: response::DailyBar,
    instrument_id: String,
) -> Result<Option<Bar>, DataProviderError> {
    let (Some(adj_open), Some(adj_high), Some(adj_low), Some(adj_close)) =
        (d.adj_open, d.adj_high, d.adj_low, d.adj_close)
    else {
        return Ok(None);
    };

    let date = NaiveDate::parse_from_str(&d.date, "%Y-%m-%d")
        .map_err(|e| DataProviderError::Parse(format!("invalid date '{}': {e}", d.date)))?;

    let timestamp = Utc.from_utc_datetime(
        &date
            .and_hms_opt(0, 0, 0)
            .ok_or_else(|| DataProviderError::Parse("invalid time".to_string()))?,
    );

    Ok(Some(Bar {
        instrument_id,
        timeframe: Timeframe::Daily,
        timestamp,
        open: JQuantsClient::to_decimal(adj_open)?,
        high: JQuantsClient::to_decimal(adj_high)?,
        low: JQuantsClient::to_decimal(adj_low)?,
        close: JQuantsClient::to_decimal(adj_close)?,
        volume: d.adj_volume.map(|v| v.round() as i64).unwrap_or(0),
    }))
}
