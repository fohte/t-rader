use async_trait::async_trait;
use chrono::NaiveDate;

use super::JQuantsClient;
use super::response::{
    MarginAlertApi, MarginAlertResponse, MarginInterestApi, MarginInterestResponse,
    flexible_decimal, flexible_i64,
};
use crate::data_provider::{DataProviderError, DateRange, MarginSource, MarginSourceError};
use crate::models::margin::{MarginAlertRecord, MarginInterestRecord, PubReason};

#[async_trait]
impl MarginSource for JQuantsClient {
    async fn fetch_margin_interest(
        &self,
        date: NaiveDate,
    ) -> Result<Vec<MarginInterestRecord>, MarginSourceError> {
        Ok(JQuantsClient::fetch_margin_interest(self, date).await?)
    }

    async fn fetch_margin_alert(
        &self,
        date: NaiveDate,
    ) -> Result<Vec<MarginAlertRecord>, MarginSourceError> {
        Ok(JQuantsClient::fetch_margin_alert(self, date).await?)
    }

    /// 契約プランが手動設定されている間だけ取得できる (未設定時のレート制限は 5 req/min のため)。
    fn fetchable_range(&self, today: NaiveDate) -> Option<DateRange> {
        self.manual_plan_date_range(today)
    }
}

impl JQuantsClient {
    /// `/markets/margin-interest` から指定日の全銘柄分を取得する (日付のみ指定、code は使わない)
    pub async fn fetch_margin_interest(
        &self,
        date: NaiveDate,
    ) -> Result<Vec<MarginInterestRecord>, DataProviderError> {
        let date_str = date.format("%Y-%m-%d").to_string();
        let raw = self
            .fetch_all_pages::<MarginInterestResponse>(
                "/markets/margin-interest",
                &[("date", &date_str)],
                self.current_rate_limit(),
            )
            .await?;

        raw.into_iter().map(margin_interest_from_api).collect()
    }

    /// `/markets/margin-alert` から指定日 (PubDate) の全銘柄分を取得する (日付のみ指定)
    pub async fn fetch_margin_alert(
        &self,
        date: NaiveDate,
    ) -> Result<Vec<MarginAlertRecord>, DataProviderError> {
        let date_str = date.format("%Y-%m-%d").to_string();
        let raw = self
            .fetch_all_pages::<MarginAlertResponse>(
                "/markets/margin-alert",
                &[("date", &date_str)],
                self.current_rate_limit(),
            )
            .await?;

        raw.into_iter().map(margin_alert_from_api).collect()
    }
}

fn parse_date(s: &str) -> Result<NaiveDate, DataProviderError> {
    NaiveDate::parse_from_str(s, "%Y-%m-%d")
        .map_err(|e| DataProviderError::Parse(format!("invalid date '{s}': {e}")))
}

fn margin_interest_from_api(
    api: MarginInterestApi,
) -> Result<MarginInterestRecord, DataProviderError> {
    let iss_type = api.iss_type.parse::<i16>().map_err(|e| {
        DataProviderError::Parse(format!("invalid IssType '{}': {e}", api.iss_type))
    })?;

    Ok(MarginInterestRecord {
        date: parse_date(&api.date)?,
        code: api.code,
        iss_type,
        shrt_vol: api.shrt_vol.round() as i64,
        long_vol: api.long_vol.round() as i64,
        shrt_neg_vol: api.shrt_neg_vol.round() as i64,
        long_neg_vol: api.long_neg_vol.round() as i64,
        shrt_std_vol: api.shrt_std_vol.round() as i64,
        long_std_vol: api.long_std_vol.round() as i64,
        shrt_val: api.shrt_val.map(|v| v.round() as i64),
        long_val: api.long_val.map(|v| v.round() as i64),
        shrt_neg_val: api.shrt_neg_val.map(|v| v.round() as i64),
        long_neg_val: api.long_neg_val.map(|v| v.round() as i64),
        shrt_std_val: api.shrt_std_val.map(|v| v.round() as i64),
        long_std_val: api.long_std_val.map(|v| v.round() as i64),
    })
}

fn margin_alert_from_api(api: MarginAlertApi) -> Result<MarginAlertRecord, DataProviderError> {
    Ok(MarginAlertRecord {
        pub_date: parse_date(&api.pub_date)?,
        code: api.code,
        app_date: parse_date(&api.app_date)?,
        pub_reason: PubReason {
            restricted: api.pub_reason.restricted,
            daily_publication: api.pub_reason.daily_publication,
            monitoring: api.pub_reason.monitoring,
            restricted_by_jsf: api.pub_reason.restricted_by_jsf,
            precaution_by_jsf: api.pub_reason.precaution_by_jsf,
            unclear_or_sec_on_alert: api.pub_reason.unclear_or_sec_on_alert,
        },
        shrt_out: api.shrt_out.round() as i64,
        long_out: api.long_out.round() as i64,
        shrt_out_chg: flexible_i64(&api.shrt_out_chg),
        long_out_chg: flexible_i64(&api.long_out_chg),
        shrt_out_ratio: flexible_decimal(&api.shrt_out_ratio),
        long_out_ratio: flexible_decimal(&api.long_out_ratio),
        sl_ratio: flexible_decimal(&api.sl_ratio),
        shrt_neg_out: api.shrt_neg_out.round() as i64,
        shrt_std_out: api.shrt_std_out.round() as i64,
        long_neg_out: api.long_neg_out.round() as i64,
        long_std_out: api.long_std_out.round() as i64,
        tse_mrgn_reg_cls: api.tse_mrgn_reg_cls,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_provider::jquants::mock::{
        JQuantsMockServer, MockMarginAlertRow, MockMarginInterestRow,
    };

    #[tokio::test]
    async fn fetch_margin_interest_parses_response_including_new_value_fields() {
        let mock = JQuantsMockServer::start().await;
        let client = mock.client().expect("client");

        mock.margin_interest()
            .date("2026-09-25")
            .rows(vec![MockMarginInterestRow {
                date: "2026-09-25",
                code: "86970",
                iss_type: "1",
                shrt_vol: 100.0,
                long_vol: 200.0,
                shrt_neg_vol: 10.0,
                long_neg_vol: 20.0,
                shrt_std_vol: 90.0,
                long_std_vol: 180.0,
                shrt_val: Some(1000.0),
                long_val: Some(2000.0),
                shrt_neg_val: Some(100.0),
                long_neg_val: Some(200.0),
                shrt_std_val: Some(900.0),
                long_std_val: Some(1800.0),
            }])
            .ok()
            .await;

        let date = NaiveDate::from_ymd_opt(2026, 9, 25).expect("date");
        let result = client.fetch_margin_interest(date).await.expect("fetch ok");

        assert_eq!(
            result,
            vec![MarginInterestRecord {
                date,
                code: "86970".to_string(),
                iss_type: 1,
                shrt_vol: 100,
                long_vol: 200,
                shrt_neg_vol: 10,
                long_neg_vol: 20,
                shrt_std_vol: 90,
                long_std_vol: 180,
                shrt_val: Some(1000),
                long_val: Some(2000),
                shrt_neg_val: Some(100),
                long_neg_val: Some(200),
                shrt_std_val: Some(900),
                long_std_val: Some(1800),
            }]
        );
    }

    #[tokio::test]
    async fn fetch_margin_interest_treats_pre_switchover_value_fields_as_none() {
        let mock = JQuantsMockServer::start().await;
        let client = mock.client().expect("client");

        mock.margin_interest()
            .date("2020-01-06")
            .rows(vec![MockMarginInterestRow {
                date: "2020-01-06",
                code: "86970",
                iss_type: "2",
                shrt_vol: 100.0,
                long_vol: 200.0,
                shrt_neg_vol: 10.0,
                long_neg_vol: 20.0,
                shrt_std_vol: 90.0,
                long_std_vol: 180.0,
                shrt_val: None,
                long_val: None,
                shrt_neg_val: None,
                long_neg_val: None,
                shrt_std_val: None,
                long_std_val: None,
            }])
            .ok()
            .await;

        let date = NaiveDate::from_ymd_opt(2020, 1, 6).expect("date");
        let result = client.fetch_margin_interest(date).await.expect("fetch ok");

        assert_eq!(
            result,
            vec![MarginInterestRecord {
                date,
                code: "86970".to_string(),
                iss_type: 2,
                shrt_vol: 100,
                long_vol: 200,
                shrt_neg_vol: 10,
                long_neg_vol: 20,
                shrt_std_vol: 90,
                long_std_vol: 180,
                shrt_val: None,
                long_val: None,
                shrt_neg_val: None,
                long_neg_val: None,
                shrt_std_val: None,
                long_std_val: None,
            }]
        );
    }

    #[tokio::test]
    async fn fetch_margin_alert_treats_dash_and_asterisk_as_none() {
        let mock = JQuantsMockServer::start().await;
        let client = mock.client().expect("client");

        mock.margin_alert()
            .date("2024-02-08")
            .rows(vec![MockMarginAlertRow {
                pub_date: "2024-02-08",
                code: "27800",
                app_date: "2024-02-07",
                shrt_out: 1000.0,
                long_out: 2000.0,
                shrt_out_chg: serde_json::json!("-"),
                long_out_chg: serde_json::json!(50.0),
                shrt_out_ratio: serde_json::json!("*"),
                long_out_ratio: serde_json::json!(12.5),
                sl_ratio: serde_json::json!(30.0),
                shrt_neg_out: 100.0,
                shrt_std_out: 900.0,
                long_neg_out: 200.0,
                long_std_out: 1800.0,
                tse_mrgn_reg_cls: "001",
            }])
            .ok()
            .await;

        let date = NaiveDate::from_ymd_opt(2024, 2, 8).expect("date");
        let result = client.fetch_margin_alert(date).await.expect("fetch ok");

        assert_eq!(
            result,
            vec![MarginAlertRecord {
                pub_date: date,
                code: "27800".to_string(),
                app_date: NaiveDate::from_ymd_opt(2024, 2, 7).expect("date"),
                pub_reason: PubReason {
                    restricted: false,
                    daily_publication: true,
                    monitoring: false,
                    restricted_by_jsf: false,
                    precaution_by_jsf: false,
                    unclear_or_sec_on_alert: false,
                },
                shrt_out: 1000,
                long_out: 2000,
                shrt_out_chg: None,
                long_out_chg: Some(50),
                shrt_out_ratio: None,
                long_out_ratio: Some(rust_decimal::Decimal::try_from(12.5).expect("decimal")),
                sl_ratio: Some(rust_decimal::Decimal::try_from(30.0).expect("decimal")),
                shrt_neg_out: 100,
                shrt_std_out: 900,
                long_neg_out: 200,
                long_std_out: 1800,
                tse_mrgn_reg_cls: "001".to_string(),
            }]
        );
    }
}
