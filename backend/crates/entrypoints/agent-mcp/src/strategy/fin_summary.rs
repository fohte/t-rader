//! 戦略実行 MCP の `read_fin_summary` tool。財務情報を返却 DTO に変換する。

use core_application::strategy_scope::StrategyScope;
use core_domain::financial_summary::FinancialSummary;
use rmcp::ErrorData as McpError;

use super::dto::{FinSummaryDto, ReadFinSummaryParams, ReadFinSummaryResult};
use super::{StrategyServer, clamp_limit, financial_summary_error};

impl StrategyServer {
    pub(crate) async fn read_fin_summary_inner(
        &self,
        // 財務情報は会社単位の開示であり戦略に属さないため検索条件に使わない
        scope: impl Into<StrategyScope>,
        params: ReadFinSummaryParams,
    ) -> Result<ReadFinSummaryResult, McpError> {
        let summaries = self
            .dependencies
            .financial_summaries
            .find_for_symbol(scope.into(), &params.symbol, clamp_limit(params.limit))
            .await
            .map_err(financial_summary_error)?;

        Ok(ReadFinSummaryResult {
            items: summaries
                .into_iter()
                .map(fin_summary_dto_from_row)
                .collect(),
        })
    }
}

fn fin_summary_dto_from_row(row: FinancialSummary) -> FinSummaryDto {
    let is_quarterly_financial_statement = is_quarterly_financial_statement(
        row.document_type.as_deref(),
        row.current_period_type.as_deref(),
    );

    FinSummaryDto {
        disc_date: row.disclosure_date,
        doc_type: row.document_type,
        current_period_type: row.current_period_type,
        current_period_start: row.current_period_start,
        current_period_end: row.current_period_end,
        current_fiscal_year_start: row.current_fiscal_year_start,
        current_fiscal_year_end: row.current_fiscal_year_end,
        sales: row.sales,
        operating_profit: row.operating_profit,
        ordinary_profit: row.ordinary_profit,
        net_profit: row.net_profit,
        sales_progress_rate: progress_rate(
            is_quarterly_financial_statement,
            row.sales,
            row.forecast_sales,
        ),
        operating_profit_progress_rate: progress_rate(
            is_quarterly_financial_statement,
            row.operating_profit,
            row.forecast_operating_profit,
        ),
        ordinary_profit_progress_rate: progress_rate(
            is_quarterly_financial_statement,
            row.ordinary_profit,
            row.forecast_ordinary_profit,
        ),
        net_profit_progress_rate: progress_rate(
            is_quarterly_financial_statement,
            row.net_profit,
            row.forecast_net_profit,
        ),
        eps: row.eps,
        bps: row.bps,
        total_assets: row.total_assets,
        equity: row.equity,
        equity_to_asset_ratio: row.equity_to_asset_ratio,
        roe: row.roe,
        cf_operating: row.cash_flow_operating,
        cf_investing: row.cash_flow_investing,
        cf_financing: row.cash_flow_financing,
        cash_and_equivalents: row.cash_and_equivalents,
        dividend_annual: row.dividend_annual,
        dividend_annual_forecast: row.dividend_annual_forecast,
        dividend_annual_forecast_next: row.dividend_annual_forecast_next,
        forecast_sales: row.forecast_sales,
        forecast_operating_profit: row.forecast_operating_profit,
        forecast_ordinary_profit: row.forecast_ordinary_profit,
        forecast_net_profit: row.forecast_net_profit,
        forecast_eps: row.forecast_eps,
        next_forecast_sales: row.next_forecast_sales,
        next_forecast_operating_profit: row.next_forecast_operating_profit,
        next_forecast_ordinary_profit: row.next_forecast_ordinary_profit,
        next_forecast_net_profit: row.next_forecast_net_profit,
        next_forecast_eps: row.next_forecast_eps,
    }
}

fn is_quarterly_financial_statement(doc_type: Option<&str>, period_type: Option<&str>) -> bool {
    let prefix = match period_type {
        Some("1Q") => "1QFinancialStatements_",
        Some("2Q") => "2QFinancialStatements_",
        Some("3Q") => "3QFinancialStatements_",
        _ => return false,
    };

    doc_type.is_some_and(|doc_type| doc_type.starts_with(prefix))
}

fn progress_rate(
    is_quarterly_financial_statement: bool,
    actual: Option<f64>,
    forecast: Option<f64>,
) -> Option<f64> {
    if !is_quarterly_financial_statement {
        return None;
    }

    let actual = actual.filter(|value| value.is_finite())?;
    let forecast = forecast.filter(|value| value.is_finite() && *value > 0.0)?;
    let rate = actual / forecast;

    rate.is_finite().then_some(rate)
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;
    use rstest::rstest;

    use core_domain::financial_summary::FinancialSummary;

    use super::{
        FinSummaryDto, fin_summary_dto_from_row, is_quarterly_financial_statement, progress_rate,
    };

    fn date(year: i32, month: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(year, month, day).expect("valid date")
    }

    fn empty_summary(disclosure_date: NaiveDate) -> FinancialSummary {
        FinancialSummary {
            code: "ZZ999".into(),
            disclosure_no: "synthetic-1".into(),
            disclosure_date,
            report_group_key: "synthetic-report".into(),
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
        }
    }

    fn empty_dto(disclosure_date: NaiveDate) -> FinSummaryDto {
        FinSummaryDto {
            disc_date: disclosure_date,
            doc_type: None,
            current_period_type: None,
            current_period_start: None,
            current_period_end: None,
            current_fiscal_year_start: None,
            current_fiscal_year_end: None,
            sales: None,
            operating_profit: None,
            ordinary_profit: None,
            net_profit: None,
            sales_progress_rate: None,
            operating_profit_progress_rate: None,
            ordinary_profit_progress_rate: None,
            net_profit_progress_rate: None,
            eps: None,
            bps: None,
            total_assets: None,
            equity: None,
            equity_to_asset_ratio: None,
            roe: None,
            cf_operating: None,
            cf_investing: None,
            cf_financing: None,
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
        }
    }

    #[test]
    fn maps_quarterly_financial_summary_to_mcp_dto() {
        let disclosure_date = date(2042, 4, 17);
        let summary = FinancialSummary {
            document_type: Some("1QFinancialStatements_Example".into()),
            current_period_type: Some("1Q".into()),
            sales: Some(25.0),
            forecast_sales: Some(100.0),
            ..empty_summary(disclosure_date)
        };

        assert_eq!(
            fin_summary_dto_from_row(summary),
            FinSummaryDto {
                doc_type: Some("1QFinancialStatements_Example".into()),
                current_period_type: Some("1Q".into()),
                sales: Some(25.0),
                forecast_sales: Some(100.0),
                sales_progress_rate: Some(0.25),
                ..empty_dto(disclosure_date)
            },
        );
    }

    #[rstest]
    #[case::first_quarter(Some("1QFinancialStatements_Example"), Some("1Q"), true)]
    #[case::second_quarter(Some("2QFinancialStatements_Example"), Some("2Q"), true)]
    #[case::third_quarter(Some("3QFinancialStatements_Example"), Some("3Q"), true)]
    #[case::annual_statement(Some("FYFinancialStatements_Example"), Some("FY"), false)]
    #[case::forecast_revision(Some("Example"), Some("1Q"), false)]
    #[case::missing_document_type(None, Some("1Q"), false)]
    fn identifies_quarterly_financial_statements(
        #[case] doc_type: Option<&str>,
        #[case] period_type: Option<&str>,
        #[case] expected: bool,
    ) {
        assert_eq!(
            is_quarterly_financial_statement(doc_type, period_type),
            expected
        );
    }

    #[rstest]
    #[case::quarterly_cumulative_actual(true, Some(50.0), Some(100.0), Some(0.5))]
    #[case::missing_actual(true, None, Some(100.0), None)]
    #[case::missing_forecast(true, Some(50.0), None, None)]
    #[case::non_positive_forecast(true, Some(50.0), Some(0.0), None)]
    #[case::annual_statement(false, Some(50.0), Some(100.0), None)]
    fn calculates_progress_rate(
        #[case] is_quarterly_financial_statement: bool,
        #[case] actual: Option<f64>,
        #[case] forecast: Option<f64>,
        #[case] expected: Option<f64>,
    ) {
        assert_eq!(
            progress_rate(is_quarterly_financial_statement, actual, forecast),
            expected
        );
    }
}
