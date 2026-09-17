//! 空売り残高報告・業種別空売り比率の取得 (`JQuantsClient` の inherent メソッド)。
//!
//! IBKR に対応するデータが無いため `DataProvider` trait には追加しない。

use chrono::NaiveDate;

use super::JQuantsClient;
use super::response::{ShortRatioResponse, ShortSaleReportResponse};
use crate::data_provider::DataProviderError;
use crate::models::{ShortRatio, ShortSaleReport};

impl JQuantsClient {
    /// `/markets/short-sale-report` を呼び出し、指定公表日の全銘柄分を取得する
    pub(crate) async fn fetch_short_sale_reports(
        &self,
        disc_date: NaiveDate,
    ) -> Result<Vec<ShortSaleReport>, DataProviderError> {
        let disc_date_str = disc_date.format("%Y-%m-%d").to_string();
        let params = [("disc_date", disc_date_str.as_str())];

        let raw_records = self
            .fetch_all_pages::<ShortSaleReportResponse>(
                "/markets/short-sale-report",
                &params,
                self.current_rate_limit(),
            )
            .await?;

        let mut reports = Vec::with_capacity(raw_records.len());
        for r in raw_records {
            let disc_date = NaiveDate::parse_from_str(&r.disc_date, "%Y-%m-%d").map_err(|e| {
                DataProviderError::Parse(format!("invalid disc_date '{}': {e}", r.disc_date))
            })?;
            let calc_date = NaiveDate::parse_from_str(&r.calc_date, "%Y-%m-%d").map_err(|e| {
                DataProviderError::Parse(format!("invalid calc_date '{}': {e}", r.calc_date))
            })?;
            let prev_report_date = if r.prev_report_date.is_empty() || r.prev_report_date == "-" {
                None
            } else {
                Some(
                    NaiveDate::parse_from_str(&r.prev_report_date, "%Y-%m-%d").map_err(|e| {
                        DataProviderError::Parse(format!(
                            "invalid prev_report_date '{}': {e}",
                            r.prev_report_date
                        ))
                    })?,
                )
            };

            reports.push(ShortSaleReport {
                disc_date,
                calc_date,
                code: r.code,
                ss_name: r.ss_name,
                ss_addr: r.ss_addr,
                dic_name: r.dic_name,
                dic_addr: r.dic_addr,
                fund_name: r.fund_name,
                short_position_ratio: Self::to_decimal(r.short_position_ratio)?,
                short_position_shares: r.short_position_shares.round() as i64,
                short_position_units: r.short_position_units.round() as i64,
                prev_report_date,
                prev_report_ratio: r.prev_report_ratio.map(Self::to_decimal).transpose()?,
                notes: r.notes,
            });
        }

        Ok(reports)
    }

    /// `/markets/short-ratio` を呼び出し、指定日の全 33 業種分を取得する
    pub(crate) async fn fetch_short_ratios(
        &self,
        date: NaiveDate,
    ) -> Result<Vec<ShortRatio>, DataProviderError> {
        let date_str = date.format("%Y-%m-%d").to_string();
        let params = [("date", date_str.as_str())];

        let raw_records = self
            .fetch_all_pages::<ShortRatioResponse>(
                "/markets/short-ratio",
                &params,
                self.current_rate_limit(),
            )
            .await?;

        let mut ratios = Vec::with_capacity(raw_records.len());
        for r in raw_records {
            let date = NaiveDate::parse_from_str(&r.date, "%Y-%m-%d")
                .map_err(|e| DataProviderError::Parse(format!("invalid date '{}': {e}", r.date)))?;

            ratios.push(ShortRatio {
                date,
                sector33_code: r.s33,
                sell_excluding_short_value: r
                    .sell_excluding_short_value
                    .map(Self::to_decimal)
                    .transpose()?,
                short_with_restriction_value: r
                    .short_with_restriction_value
                    .map(Self::to_decimal)
                    .transpose()?,
                short_without_restriction_value: r
                    .short_without_restriction_value
                    .map(Self::to_decimal)
                    .transpose()?,
            });
        }

        Ok(ratios)
    }
}

#[cfg(test)]
mod tests {
    use rust_decimal::Decimal;

    use super::*;
    use crate::data_provider::jquants::mock::{JQuantsMockServer, MockShortSaleReport};

    #[tokio::test]
    async fn fetch_short_sale_reports_treats_hyphen_prev_report_date_as_none() {
        let mock = JQuantsMockServer::start().await;
        let client = mock.client().expect("client");

        mock.short_sale_report()
            .disc_date("2016-09-20")
            .reports(vec![MockShortSaleReport {
                code: "7203",
                ss_name: "テスト証券",
                short_position_ratio: 0.01,
                prev_report_date: "-",
            }])
            .ok()
            .await;

        let disc_date = NaiveDate::from_ymd_opt(2016, 9, 20).expect("date");
        let result = client
            .fetch_short_sale_reports(disc_date)
            .await
            .expect("fetch ok");

        assert_eq!(
            result,
            vec![ShortSaleReport {
                disc_date,
                calc_date: disc_date,
                code: "7203".to_string(),
                ss_name: "テスト証券".to_string(),
                ss_addr: "テスト住所".to_string(),
                dic_name: "テスト委託者".to_string(),
                dic_addr: "テスト住所".to_string(),
                fund_name: String::new(),
                short_position_ratio: Decimal::try_from(0.01).expect("decimal"),
                short_position_shares: 1000,
                short_position_units: 10,
                prev_report_date: None,
                prev_report_ratio: None,
                notes: String::new(),
            }]
        );
    }
}
