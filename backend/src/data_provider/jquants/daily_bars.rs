use async_trait::async_trait;
use chrono::{NaiveDate, Utc};

use super::response::DailyBarsResponse;
use super::{JQuantsClient, normalize_local_code, parse_daily_bar};
use crate::data_provider::{
    DailyBarSource, DailyBarSourceError, DataProviderError, DateRange, MarketDailyBarSource,
    MarketDailyBarSourceError,
};
use crate::models::bar::Bar;

impl JQuantsClient {
    /// `/equities/bars/daily` を実際に呼び出す (契約範囲外エラーの自己修復はしない)
    async fn fetch_daily_bars_for_instrument(
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

    pub fn known_fetchable_range(&self) -> Option<(NaiveDate, NaiveDate)> {
        let range = self.known_fetchable_date_range(Utc::now().date_naive());
        Some((range.from, range.to))
    }
}

#[async_trait]
impl DailyBarSource for JQuantsClient {
    async fn fetch_daily_bars(
        &self,
        instrument_id: &str,
        range: &DateRange,
    ) -> Result<Vec<Bar>, DailyBarSourceError> {
        JQuantsClient::fetch_daily_bars_for_instrument(self, instrument_id, range)
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

    fn fetchable_range(&self, today: NaiveDate) -> Option<DateRange> {
        Some(self.plan_date_range(today))
    }
}
