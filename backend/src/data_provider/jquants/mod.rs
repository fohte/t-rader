#[cfg(test)]
pub(crate) mod mock;
mod response;
#[cfg(test)]
mod tests;

use std::collections::VecDeque;

use chrono::{Duration, NaiveDate, TimeZone, Utc};
use reqwest::Url;
use rust_decimal::Decimal;
use tokio::sync::Mutex;

use crate::data_provider::{DataProvider, DataProviderError, DateRange};
use crate::models::bar::{Bar, Timeframe};
use crate::models::instrument::{Instrument, Market};
use crate::models::jquants_plan::JQuantsPlan;
use response::{
    DailyBarsResponse, EdinetDocumentsResponse, EquitiesMasterResponse, ErrorResponse,
    FinSummaryResponse, Paginated,
};

const DEFAULT_BASE_URL: &str = "https://api.jquants.com/v2";
const MAX_RETRIES: u32 = 3;
const INITIAL_BACKOFF_MS: u64 = 500;
/// API サーバーのバグで同じ pagination_key が返り続けた場合の安全策
const MAX_PAGES: u32 = 100;

/// レートリミットのウィンドウ幅 (60 秒)
const RATE_LIMIT_WINDOW: std::time::Duration = std::time::Duration::from_secs(60);
/// 契約プラン未設定時のウィンドウ内最大リクエスト数 (J-Quants 無料プラン相当)。
/// 上限を実際の契約より高く見積もると 429 のリトライでは済まない大幅な超過に
/// つながるため、未設定時は最も低い Free プランの値に倒す。
const RATE_LIMIT_MAX_REQUESTS: usize = 5;

/// `/fins/summary` (財務情報) 固有のレート制限 (契約プランと別枠、公式ページ記載の値)。
/// 大幅に超過すると 5 分程度アクセスが完全に遮断されるため、契約プラン上限より低い方を使う。
const FIN_SUMMARY_RATE_LIMIT_PER_MINUTE: usize = 60;

/// スライディングウィンドウ方式のレートリミッター
///
/// 直近 60 秒間のリクエスト送信時刻を記録し、上限に達している場合は
/// 最も古いリクエストがウィンドウから外れるまで待機する。
struct RateLimiter {
    /// 直近のリクエスト送信時刻 (古い順)
    timestamps: Mutex<VecDeque<tokio::time::Instant>>,
}

impl RateLimiter {
    fn new() -> Self {
        Self {
            timestamps: Mutex::new(VecDeque::with_capacity(RATE_LIMIT_MAX_REQUESTS)),
        }
    }

    /// リクエスト送信の許可を取得する
    ///
    /// ウィンドウ内のリクエスト数が `max_requests` に達している場合、最も古い
    /// リクエストがウィンドウから外れるまで待機する。`max_requests` は契約プランに
    /// 応じて呼び出しごとに変わりうる (`JQuantsClient::current_rate_limit`)。
    async fn acquire(&self, max_requests: usize) {
        loop {
            let now = tokio::time::Instant::now();

            let mut timestamps = self.timestamps.lock().await;

            // ウィンドウ外のタイムスタンプを削除
            while let Some(&oldest) = timestamps.front() {
                if now.duration_since(oldest) >= RATE_LIMIT_WINDOW {
                    timestamps.pop_front();
                } else {
                    break;
                }
            }

            if timestamps.len() < max_requests {
                // 枠がある: タイムスタンプを記録して通過
                timestamps.push_back(now);
                return;
            }

            // 枠がない: 最も古いリクエストがウィンドウから外れるまで待つ
            let oldest = timestamps[0];
            let sleep_target = oldest + RATE_LIMIT_WINDOW;
            drop(timestamps); // ロックを解放してから sleep

            tracing::info!(
                max_requests,
                wait_ms = sleep_target.saturating_duration_since(now).as_millis() as u64,
                "レートリミットに到達、待機中"
            );
            tokio::time::sleep_until(sleep_target).await;
        }
    }
}

/// 400 エラーメッセージから検出した契約範囲。プラン変更や日々のローリング
/// ウィンドウにより実際の範囲は動きうるため、`DETECTED_RANGE_TTL_DAYS` を
/// 超えたら期限切れとして扱い、再検出を促す。
struct DetectedRange {
    from: NaiveDate,
    to: NaiveDate,
    detected_at: NaiveDate,
}

/// 検出済みの契約範囲を再検出なしで使い回せる期間 (日数)
const DETECTED_RANGE_TTL_DAYS: i64 = 1;

/// `detected` が `today` 時点でまだ有効かどうかを判定し、有効なら範囲を返す。
/// TTL を超えていれば `None` を返し、呼び出し側に再検出を促す。
fn effective_range(
    detected: Option<&DetectedRange>,
    today: NaiveDate,
) -> Option<(NaiveDate, NaiveDate)> {
    let d = detected?;
    (today - d.detected_at < Duration::days(DETECTED_RANGE_TTL_DAYS)).then_some((d.from, d.to))
}

/// J-Quants API V2 クライアント
///
/// API Key 認証方式で J-Quants API V2 にアクセスする。
/// アプリケーションレベルのレートリミッターを内蔵し、429 (Rate Limited) と
/// 5xx に対して指数バックオフでリトライする。レートリミッターの上限は契約プラン
/// (`manual_plan`) に追従し、未設定時は Free プラン相当に倒す。
///
/// Debug は意図的に derive しない (api_key の漏洩防止)
pub struct JQuantsClient {
    http: reqwest::Client,
    base_url: String,
    api_key: String,
    rate_limiter: RateLimiter,
    detected_range: std::sync::Mutex<Option<DetectedRange>>,
    /// 設定ページから手動設定された契約プラン。`None` の間は自動検出
    /// (`detected_range`) を使う。
    manual_plan: std::sync::Mutex<Option<JQuantsPlan>>,
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
            rate_limiter: RateLimiter::new(),
            detected_range: std::sync::Mutex::new(None),
            manual_plan: std::sync::Mutex::new(None),
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
            rate_limiter: RateLimiter::new(),
            detected_range: std::sync::Mutex::new(None),
            manual_plan: std::sync::Mutex::new(None),
        })
    }

    /// 設定ページからの手動プラン設定を反映する。プロセス再起動なしで即座に
    /// `known_fetchable_range()` の結果へ反映される。
    pub fn set_manual_plan(&self, plan: Option<JQuantsPlan>) {
        let mut guard = self.manual_plan.lock().unwrap_or_else(|e| e.into_inner());
        *guard = plan;
    }

    /// 財務情報の取り込み (`services::fin_summary_ingest`) が、契約プラン未設定の間は
    /// 取り込みをスキップする判定に使う。
    pub(crate) fn manual_plan(&self) -> Option<JQuantsPlan> {
        let guard = self.manual_plan.lock().unwrap_or_else(|e| e.into_inner());
        *guard
    }

    /// レートリミッターの現在の上限 (1 分あたりのリクエスト数)
    ///
    /// 契約プランが設定されていればそのプランの値、未設定なら
    /// `RATE_LIMIT_MAX_REQUESTS` (Free プラン相当) を返す。
    fn current_rate_limit(&self) -> usize {
        self.manual_plan()
            .map(|plan| plan.rate_limit_per_minute())
            .unwrap_or(RATE_LIMIT_MAX_REQUESTS)
    }

    fn detected_range(&self) -> Option<(NaiveDate, NaiveDate)> {
        let guard = self
            .detected_range
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        effective_range(guard.as_ref(), Utc::now().date_naive())
    }

    /// crate 内テスト (`services::edinet_holdings` 等) から 400 検出フローを経由せず
    /// 狭い範囲を直接設定できるように、crate 内に可視性を広げている。
    pub(crate) fn set_detected_range(&self, range: (NaiveDate, NaiveDate)) {
        let mut guard = self
            .detected_range
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        *guard = Some(DetectedRange {
            from: range.0,
            to: range.1,
            detected_at: Utc::now().date_naive(),
        });
    }

    /// 指数バックオフ付き GET リクエスト
    ///
    /// レートリミッターで送信間隔を制御した上で、429 と 5xx に対してリトライする。
    /// それ以外のエラーは即座に返す。`max_requests` はウィンドウ内の許容リクエスト数
    /// (通常は `current_rate_limit()`。エンドポイント固有の上限がある場合はそれとの min)。
    async fn get_with_retry(
        &self,
        url: &Url,
        max_requests: usize,
    ) -> Result<reqwest::Response, DataProviderError> {
        let mut last_error = None;
        let url_str = url.as_str();

        for attempt in 0..=MAX_RETRIES {
            // 各リクエスト (リトライ含む) の前にレートリミッターの許可を取得
            self.rate_limiter.acquire(max_requests).await;

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

    /// `/equities/bars/daily` を実際に呼び出す (契約範囲外エラーの自己修復はしない)
    async fn fetch_daily_bars_once(
        &self,
        instrument_id: &str,
        range: &DateRange,
    ) -> Result<Vec<Bar>, DataProviderError> {
        let from_str = range.from.format("%Y%m%d").to_string();
        let to_str = range.to.format("%Y%m%d").to_string();
        let params = [
            ("code", instrument_id),
            ("from", &from_str),
            ("to", &to_str),
        ];

        let raw_bars = self
            .fetch_all_pages::<DailyBarsResponse>(
                "/equities/bars/daily",
                &params,
                self.current_rate_limit(),
            )
            .await?;

        let mut all_bars = Vec::with_capacity(raw_bars.len());
        for d in raw_bars {
            // 調整後価格が null のレコードはスキップ (非取引日等)
            let (Some(adj_open), Some(adj_high), Some(adj_low), Some(adj_close)) =
                (d.adj_open, d.adj_high, d.adj_low, d.adj_close)
            else {
                continue;
            };

            let date = NaiveDate::parse_from_str(&d.date, "%Y-%m-%d")
                .map_err(|e| DataProviderError::Parse(format!("invalid date '{}': {e}", d.date)))?;

            let timestamp = Utc.from_utc_datetime(
                &date
                    .and_hms_opt(0, 0, 0)
                    .ok_or_else(|| DataProviderError::Parse("invalid time".to_string()))?,
            );

            all_bars.push(Bar {
                // API レスポンスの Code (5 桁) ではなく、引数の instrument_id (4 桁) を使う
                instrument_id: instrument_id.to_string(),
                timeframe: Timeframe::Daily,
                timestamp,
                open: Self::to_decimal(adj_open)?,
                high: Self::to_decimal(adj_high)?,
                low: Self::to_decimal(adj_low)?,
                close: Self::to_decimal(adj_close)?,
                volume: d.adj_volume.map(|v| v.round() as i64).unwrap_or(0),
            });
        }

        all_bars.sort_by_key(|b| b.timestamp);
        Ok(all_bars)
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
            .min(FIN_SUMMARY_RATE_LIMIT_PER_MINUTE);

        self.fetch_all_pages::<FinSummaryResponse>("/fins/summary", &params, max_requests)
            .await
    }
}

impl DataProvider for JQuantsClient {
    /// 契約範囲外エラー (400) 発生時は契約範囲を検出し、その範囲でこの呼び出し内で
    /// 1 回だけ再試行する (検出済み範囲外の日付を再度指定すれば何度でも発動しうる)。
    async fn fetch_daily_bars(
        &self,
        instrument_id: &str,
        range: &DateRange,
    ) -> Result<Vec<Bar>, DataProviderError> {
        match self.fetch_daily_bars_once(instrument_id, range).await {
            Err(DataProviderError::Api {
                status: 400,
                message,
            }) => {
                let Some((from, to)) = parse_subscription_range(&message) else {
                    return Err(DataProviderError::Api {
                        status: 400,
                        message,
                    });
                };
                tracing::info!(
                    instrument_id,
                    %from,
                    %to,
                    "契約範囲を検出しました。検出した範囲で再取得します"
                );
                self.set_detected_range((from, to));
                self.fetch_daily_bars_once(instrument_id, &DateRange { from, to })
                    .await
            }
            other => other,
        }
    }

    /// 手動設定 (設定ページ) が優先。未設定なら 400 エラーからの自動検出結果を使う。
    fn known_fetchable_range(&self) -> Option<(NaiveDate, NaiveDate)> {
        match self.manual_plan() {
            Some(plan) => Some(plan.range(Utc::now().date_naive())),
            None => self.detected_range(),
        }
    }

    /// 手動設定 (推定して確定した後の値も含む) が既にあるなら何もしない。これが「初回だけ」
    /// であることを保証する。まだ何も検出されていない、または DB 側で既に設定済み
    /// (他プロセス/リクエストが先に推定・永続化した等) の場合も何もしない。
    async fn persist_inferred_range_if_needed(
        &self,
        db: &sea_orm::DatabaseConnection,
    ) -> Result<(), DataProviderError> {
        if self.manual_plan().is_some() {
            return Ok(());
        }
        let Some(range) = self.detected_range() else {
            return Ok(());
        };

        let inferred = JQuantsPlan::infer_from_range(range);
        let data = crate::models::JQuantsPlanSettingData {
            schema_version: crate::models::jquants_plan::JQUANTS_PLAN_SETTING_SCHEMA_VERSION,
            plan: Some(inferred),
        };
        let value = crate::models::serialize_plan_setting(&data)
            .map_err(|e| DataProviderError::Database(e.to_string()))?;
        let saved = crate::services::jquants_plan_setting::save_if_unset(db, value)
            .await
            .map_err(|e| DataProviderError::Database(e.to_string()))?;
        if !saved {
            // 手動設定 (PUT) と競合し、既に設定済みだったため何もしない
            return Ok(());
        }

        self.set_manual_plan(Some(inferred));
        tracing::info!(
            ?inferred,
            from = %range.0,
            to = %range.1,
            "契約範囲を検出したためプランを推定し、設定として永続化しました (初回のみ)"
        );
        Ok(())
    }

    async fn fetch_instrument(&self, instrument_id: &str) -> Result<Instrument, DataProviderError> {
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
        })
    }
}

/// J-Quants API が契約範囲外の日付を指定されたときに返す 400 エラーメッセージから
/// 契約範囲を抽出する。想定する message の例 (日付は形式を示すための架空の値):
/// "Your subscription covers the following dates: 2020-04-01 ~ 2022-04-01. ..."
fn parse_subscription_range(message: &str) -> Option<(NaiveDate, NaiveDate)> {
    let after_marker = message.split("covers the following dates:").nth(1)?;
    let mut dates = after_marker.split_whitespace().filter_map(|token| {
        let cleaned = token.trim_matches(|c: char| !c.is_ascii_digit() && c != '-');
        NaiveDate::parse_from_str(cleaned, "%Y-%m-%d").ok()
    });
    let from = dates.next()?;
    let to = dates.next()?;
    Some((from, to))
}
