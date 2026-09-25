use async_trait::async_trait;
use chrono::{NaiveDate, Utc};

use super::response::DailyBarsResponse;
use super::{JQuantsClient, normalize_local_code, parse_daily_bar};
use crate::data_provider::{
    DailyBarSource, DailyBarSourceError, DataProviderError, DateRange, MarketDailyBarSource,
    MarketDailyBarSourceError,
};
use crate::models::bar::Bar;
use crate::models::jquants_plan::JQuantsPlan;

impl JQuantsClient {
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
            // API レスポンスの Code (5 桁) ではなく、引数の instrument_id (4 桁) を使う
            if let Some(bar) = parse_daily_bar(d, instrument_id.to_string())? {
                all_bars.push(bar);
            }
        }

        all_bars.sort_by_key(|b| b.timestamp);
        Ok(all_bars)
    }

    /// `/equities/bars/daily` を `date` のみ指定して呼び出し、その日の全上場銘柄分の
    /// 日足をまとめて取得する。銘柄コードはレスポンスの 5 桁 Code から正規化する。
    pub(crate) async fn fetch_daily_bars_by_date(
        &self,
        date: NaiveDate,
    ) -> Result<Vec<Bar>, DataProviderError> {
        let date_str = date.format("%Y-%m-%d").to_string();
        let params = [("date", date_str.as_str())];

        let raw_bars = self
            .fetch_all_pages::<DailyBarsResponse>(
                "/equities/bars/daily",
                &params,
                self.current_rate_limit(),
            )
            .await?;

        let mut all_bars = Vec::with_capacity(raw_bars.len());
        for d in raw_bars {
            let instrument_id = normalize_local_code(&d.code).to_string();
            if let Some(bar) = parse_daily_bar(d, instrument_id)? {
                all_bars.push(bar);
            }
        }

        Ok(all_bars)
    }

    /// 契約範囲外エラー (400) 発生時は契約範囲を検出し、その範囲でこの呼び出し内で
    /// 1 回だけ再試行する。DB が設定されていれば、取得結果にかかわらず検出範囲をプランとして保存する。
    pub(super) async fn fetch_daily_bars_with_range_detection(
        &self,
        instrument_id: &str,
        range: &DateRange,
    ) -> Result<Vec<Bar>, DataProviderError> {
        let result = match self.fetch_daily_bars_once(instrument_id, range).await {
            Err(DataProviderError::Api {
                status: 400,
                message,
            }) => match parse_subscription_range(&message) {
                Some((from, to)) => {
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
                None => Err(DataProviderError::Api {
                    status: 400,
                    message,
                }),
            },
            other => other,
        };

        if let Err(error) = self.persist_inferred_range_if_needed().await {
            tracing::warn!(error = %error, "契約プランの推定結果の永続化に失敗しました");
        }

        result
    }

    /// 手動設定 (設定ページ) が優先。未設定なら 400 エラーからの自動検出結果を使う。
    pub fn known_fetchable_range(&self) -> Option<(NaiveDate, NaiveDate)> {
        match self.manual_plan() {
            Some(plan) => Some(plan.range(Utc::now().date_naive())),
            None => self.detected_range(),
        }
    }

    /// 手動設定 (推定して確定した後の値も含む) が既にあるなら何もしない。これが「初回だけ」
    /// であることを保証する。まだ何も検出されていない、または DB 側で既に設定済み
    /// (他プロセス/リクエストが先に推定・永続化した等) の場合も何もしない。
    pub(super) async fn persist_inferred_range_if_needed(&self) -> Result<(), DataProviderError> {
        let Some(db) = &self.db else {
            return Ok(());
        };
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
}

#[async_trait]
impl DailyBarSource for JQuantsClient {
    async fn fetch_daily_bars(
        &self,
        instrument_id: &str,
        range: &DateRange,
    ) -> Result<Vec<Bar>, DailyBarSourceError> {
        JQuantsClient::fetch_daily_bars_with_range_detection(self, instrument_id, range)
            .await
            .map_err(Into::into)
    }

    fn known_fetchable_range(&self) -> Option<(NaiveDate, NaiveDate)> {
        JQuantsClient::known_fetchable_range(self)
    }
}

#[async_trait]
impl MarketDailyBarSource for JQuantsClient {
    async fn fetch_daily_bars_by_date(
        &self,
        date: NaiveDate,
    ) -> Result<Vec<Bar>, MarketDailyBarSourceError> {
        JQuantsClient::fetch_daily_bars_by_date(self, date)
            .await
            .map_err(Into::into)
    }

    /// 契約プランが手動設定されている間だけ取得できる (未設定時のレート制限は 5 req/分で、
    /// 全銘柄分の取得には数百リクエストを要するため)。
    fn fetchable_range(&self, today: NaiveDate) -> Option<DateRange> {
        self.manual_plan_date_range(today)
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
