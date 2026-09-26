use async_trait::async_trait;
use chrono::NaiveDate;
use core_domain::financial_summary::FinancialSummary;

use super::JQuantsClient;
use crate::data_provider::{
    DataProviderError, DateRange, FinancialSummarySource, FinancialSummarySourceError,
};

#[async_trait]
impl FinancialSummarySource for JQuantsClient {
    async fn fetch_financial_summaries_by_date(
        &self,
        date: NaiveDate,
    ) -> Result<Vec<FinancialSummary>, FinancialSummarySourceError> {
        self.fetch_fin_summary_by_date(date)
            .await?
            .into_iter()
            .map(financial_summary_from_api)
            .collect::<Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    fn fetchable_range(&self, today: NaiveDate) -> Option<DateRange> {
        Some(self.plan_date_range(today))
    }
}

fn financial_summary_from_api(
    raw: serde_json::Value,
) -> Result<FinancialSummary, DataProviderError> {
    let required_string = |key: &str| {
        raw.get(key)
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| {
                DataProviderError::Parse(format!("fin summary response missing '{key}'"))
            })
    };
    let string_field = |key: &str| {
        raw.get(key)
            .and_then(serde_json::Value::as_str)
            .filter(|value| !value.is_empty())
    };
    let date_field = |key: &str| {
        string_field(key).and_then(|value| {
            NaiveDate::parse_from_str(value, "%Y-%m-%d")
                .map_err(|error| {
                    tracing::warn!(
                        code = raw.get("Code").and_then(serde_json::Value::as_str).unwrap_or_default(),
                        disclosure_no = raw
                            .get("DiscNo")
                            .and_then(serde_json::Value::as_str)
                            .unwrap_or_default(),
                        field = key,
                        value,
                        %error,
                        "財務情報の項目を parse できず欠損値として扱います",
                    );
                })
                .ok()
        })
    };
    let number_field = |key: &str| {
        string_field(key).and_then(|value| {
            value.parse::<f64>().map_err(|error| {
                tracing::warn!(
                    code = raw.get("Code").and_then(serde_json::Value::as_str).unwrap_or_default(),
                    disclosure_no = raw
                        .get("DiscNo")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or_default(),
                    field = key,
                    value,
                    %error,
                    "財務情報の項目を parse できず欠損値として扱います",
                );
            }).ok()
        })
    };

    let disclosure_date = NaiveDate::parse_from_str(required_string("DiscDate")?, "%Y-%m-%d")
        .map_err(|error| DataProviderError::Parse(format!("invalid DiscDate: {error}")))?;

    Ok(FinancialSummary {
        code: required_string("Code")?.to_string(),
        disclosure_no: required_string("DiscNo")?.to_string(),
        disclosure_date,
        report_group_key: report_group_key(&raw),
        document_type: string_field("DocType").map(str::to_string),
        current_period_type: string_field("CurPerType").map(str::to_string),
        current_period_start: date_field("CurPerSt"),
        current_period_end: date_field("CurPerEn"),
        current_fiscal_year_start: date_field("CurFYSt"),
        current_fiscal_year_end: date_field("CurFYEn"),
        sales: number_field("Sales"),
        operating_profit: number_field("OP"),
        ordinary_profit: number_field("OdP"),
        net_profit: number_field("NP"),
        eps: number_field("EPS"),
        bps: number_field("BPS"),
        total_assets: number_field("TA"),
        equity: number_field("Eq"),
        equity_to_asset_ratio: number_field("EqAR"),
        roe: number_field("ROE"),
        cash_flow_operating: number_field("CFO"),
        cash_flow_investing: number_field("CFI"),
        cash_flow_financing: number_field("CFF"),
        cash_and_equivalents: number_field("CashEq"),
        dividend_annual: number_field("DivAnn"),
        dividend_annual_forecast: number_field("FDivAnn"),
        dividend_annual_forecast_next: number_field("NxFDivAnn"),
        forecast_sales: number_field("FSales"),
        forecast_operating_profit: number_field("FOP"),
        forecast_ordinary_profit: number_field("FOdP"),
        forecast_net_profit: number_field("FNP"),
        forecast_eps: number_field("FEPS"),
        next_forecast_sales: number_field("NxFSales"),
        next_forecast_operating_profit: number_field("NxFOP"),
        next_forecast_ordinary_profit: number_field("NxFOdP"),
        next_forecast_net_profit: number_field("NxFNp"),
        next_forecast_eps: number_field("NxFEPS"),
    })
}

fn report_group_key(raw: &serde_json::Value) -> String {
    ["DocType", "CurPerSt", "CurPerEn"]
        .into_iter()
        .map(|key| {
            let value = raw.get(key).and_then(|value| match value {
                serde_json::Value::Null => None,
                serde_json::Value::String(value) => Some(value.clone()),
                value => Some(value.to_string()),
            });

            match value {
                Some(value) => format!("V{}:{value};", value.len()),
                None => "N;".to_string(),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;
    use core_domain::financial_summary::FinancialSummary;
    use serde_json::json;

    use super::{FinancialSummarySource, report_group_key};
    use crate::data_provider::jquants::mock::JQuantsMockServer;

    #[tokio::test]
    async fn fetches_typed_financial_summaries() {
        let actual = fetch_summaries(json!({
            "DiscDate": "2026-05-01",
            "Code": "99990",
            "DiscNo": "2",
            "DocType": "QuarterlyStatement",
            "CurPerType": "1Q",
            "CurPerSt": "2026-04-01",
            "CurPerEn": "2026-06-30",
            "CurFYSt": "2026-01-01",
            "CurFYEn": "2026-12-31",
            "Sales": "101",
            "OP": "102",
            "OdP": "103",
            "NP": "104",
            "EPS": "105",
            "BPS": "106",
            "TA": "107",
            "Eq": "108",
            "EqAR": "109",
            "ROE": "110",
            "CFO": "111",
            "CFI": "112",
            "CFF": "113",
            "CashEq": "114",
            "DivAnn": "115",
            "FDivAnn": "116",
            "NxFDivAnn": "117",
            "FSales": "118",
            "FOP": "119",
            "FOdP": "120",
            "FNP": "121",
            "FEPS": "122",
            "NxFSales": "123",
            "NxFOP": "124",
            "NxFOdP": "125",
            "NxFNp": "-126",
            "NxFEPS": "127",
        }))
        .await;

        assert_eq!(
            actual,
            vec![FinancialSummary {
                code: "99990".to_string(),
                disclosure_no: "2".to_string(),
                disclosure_date: NaiveDate::from_ymd_opt(2026, 5, 1).expect("valid date"),
                report_group_key: "V18:QuarterlyStatement;V10:2026-04-01;V10:2026-06-30;"
                    .to_string(),
                document_type: Some("QuarterlyStatement".to_string()),
                current_period_type: Some("1Q".to_string()),
                current_period_start: Some(
                    NaiveDate::from_ymd_opt(2026, 4, 1).expect("valid date")
                ),
                current_period_end: Some(NaiveDate::from_ymd_opt(2026, 6, 30).expect("valid date")),
                current_fiscal_year_start: Some(
                    NaiveDate::from_ymd_opt(2026, 1, 1).expect("valid date")
                ),
                current_fiscal_year_end: Some(
                    NaiveDate::from_ymd_opt(2026, 12, 31).expect("valid date")
                ),
                sales: Some(101.0),
                operating_profit: Some(102.0),
                ordinary_profit: Some(103.0),
                net_profit: Some(104.0),
                eps: Some(105.0),
                bps: Some(106.0),
                total_assets: Some(107.0),
                equity: Some(108.0),
                equity_to_asset_ratio: Some(109.0),
                roe: Some(110.0),
                cash_flow_operating: Some(111.0),
                cash_flow_investing: Some(112.0),
                cash_flow_financing: Some(113.0),
                cash_and_equivalents: Some(114.0),
                dividend_annual: Some(115.0),
                dividend_annual_forecast: Some(116.0),
                dividend_annual_forecast_next: Some(117.0),
                forecast_sales: Some(118.0),
                forecast_operating_profit: Some(119.0),
                forecast_ordinary_profit: Some(120.0),
                forecast_net_profit: Some(121.0),
                forecast_eps: Some(122.0),
                next_forecast_sales: Some(123.0),
                next_forecast_operating_profit: Some(124.0),
                next_forecast_ordinary_profit: Some(125.0),
                next_forecast_net_profit: Some(-126.0),
                next_forecast_eps: Some(127.0),
            }],
        );
    }

    #[tokio::test]
    async fn maps_blank_and_missing_fields_to_none() {
        let actual = fetch_summaries(json!({
            "DiscDate": "2026-05-01",
            "Code": "99990",
            "DiscNo": "2",
            "OP": "",
        }))
        .await;

        assert_eq!(
            actual,
            vec![FinancialSummary {
                code: "99990".to_string(),
                disclosure_no: "2".to_string(),
                disclosure_date: NaiveDate::from_ymd_opt(2026, 5, 1).expect("valid date"),
                report_group_key: "N;N;N;".to_string(),
                document_type: None,
                current_period_type: None,
                current_period_start: None,
                current_period_end: None,
                current_fiscal_year_start: None,
                current_fiscal_year_end: None,
                sales: None,
                operating_profit: None,
                ordinary_profit: None,
                net_profit: None,
                eps: None,
                bps: None,
                total_assets: None,
                equity: None,
                equity_to_asset_ratio: None,
                roe: None,
                cash_flow_operating: None,
                cash_flow_investing: None,
                cash_flow_financing: None,
                cash_and_equivalents: None,
                dividend_annual: None,
                dividend_annual_forecast: None,
                dividend_annual_forecast_next: None,
                forecast_sales: None,
                forecast_operating_profit: None,
                forecast_ordinary_profit: None,
                forecast_net_profit: None,
                forecast_eps: None,
                next_forecast_sales: None,
                next_forecast_operating_profit: None,
                next_forecast_ordinary_profit: None,
                next_forecast_net_profit: None,
                next_forecast_eps: None,
            }],
        );
    }

    async fn fetch_summaries(item: serde_json::Value) -> Vec<FinancialSummary> {
        let mock = JQuantsMockServer::start().await;
        let client = mock.client().expect("client");
        let date = NaiveDate::from_ymd_opt(2026, 5, 1).expect("valid date");

        mock.fin_summary()
            .date("2026-05-01")
            .items(vec![item])
            .ok()
            .await;

        client
            .fetch_financial_summaries_by_date(date)
            .await
            .expect("fetch succeeds")
    }

    #[test]
    fn report_group_key_distinguishes_empty_values_from_missing_values() {
        let actual = [
            report_group_key(&json!({"DocType": "", "CurPerSt": ""})),
            report_group_key(&json!({})),
        ];

        let expected = ["V0:;V0:;N;".to_string(), "N;N;N;".to_string()];
        assert_eq!(actual, expected);
    }
}
