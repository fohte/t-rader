use chrono::NaiveDate;

/// 銘柄ごとの開示時点の財務情報。
#[derive(Debug, Clone, PartialEq)]
pub struct FinancialSummary {
    pub code: String,
    pub disclosure_no: String,
    pub disclosure_date: NaiveDate,
    /// 開示種別と対象期間で報告グループを識別する不透明なキー。
    pub report_group_key: String,
    pub document_type: Option<String>,
    pub current_period_type: Option<String>,
    pub current_period_start: Option<NaiveDate>,
    pub current_period_end: Option<NaiveDate>,
    pub current_fiscal_year_start: Option<NaiveDate>,
    pub current_fiscal_year_end: Option<NaiveDate>,
    pub sales: Option<f64>,
    pub operating_profit: Option<f64>,
    pub ordinary_profit: Option<f64>,
    pub net_profit: Option<f64>,
    pub eps: Option<f64>,
    pub bps: Option<f64>,
    pub total_assets: Option<f64>,
    pub equity: Option<f64>,
    pub equity_to_asset_ratio: Option<f64>,
    pub roe: Option<f64>,
    pub cash_flow_operating: Option<f64>,
    pub cash_flow_investing: Option<f64>,
    pub cash_flow_financing: Option<f64>,
    pub cash_and_equivalents: Option<f64>,
    pub dividend_annual: Option<f64>,
    pub dividend_annual_forecast: Option<f64>,
    pub dividend_annual_forecast_next: Option<f64>,
    pub forecast_sales: Option<f64>,
    pub forecast_operating_profit: Option<f64>,
    pub forecast_ordinary_profit: Option<f64>,
    pub forecast_net_profit: Option<f64>,
    pub forecast_eps: Option<f64>,
    pub next_forecast_sales: Option<f64>,
    pub next_forecast_operating_profit: Option<f64>,
    pub next_forecast_ordinary_profit: Option<f64>,
    pub next_forecast_net_profit: Option<f64>,
    pub next_forecast_eps: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FinancialSummaryPeriodType {
    Fy,
    FirstQuarter,
    SecondQuarter,
    ThirdQuarter,
    FourthQuarter,
}

impl FinancialSummaryPeriodType {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Fy => "FY",
            Self::FirstQuarter => "1Q",
            Self::SecondQuarter => "2Q",
            Self::ThirdQuarter => "3Q",
            Self::FourthQuarter => "4Q",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "FY" => Some(Self::Fy),
            "1Q" => Some(Self::FirstQuarter),
            "2Q" => Some(Self::SecondQuarter),
            "3Q" => Some(Self::ThirdQuarter),
            "4Q" => Some(Self::FourthQuarter),
            _ => None,
        }
    }
}

pub fn is_actual_financial_summary_for_period(
    document_type: Option<&str>,
    current_period_type: Option<&str>,
    current_period_end: Option<NaiveDate>,
    target_period_type: FinancialSummaryPeriodType,
    target_period_end: NaiveDate,
) -> bool {
    if current_period_type != Some(target_period_type.as_str())
        || current_period_end != Some(target_period_end)
    {
        return false;
    }

    let Some(document_type) = document_type else {
        return false;
    };

    match target_period_type {
        FinancialSummaryPeriodType::Fy => document_type.starts_with("FYFinancialStatements_"),
        FinancialSummaryPeriodType::FirstQuarter => {
            document_type.starts_with("1QFinancialStatements_")
        }
        FinancialSummaryPeriodType::SecondQuarter => {
            document_type.starts_with("2QFinancialStatements_")
        }
        FinancialSummaryPeriodType::ThirdQuarter => {
            document_type.starts_with("3QFinancialStatements_")
        }
        FinancialSummaryPeriodType::FourthQuarter => {
            document_type.starts_with("OtherPeriodFinancialStatements_")
        }
    }
}

#[cfg(test)]
mod period_match_tests {
    use chrono::NaiveDate;
    use rstest::rstest;

    use super::{FinancialSummaryPeriodType, is_actual_financial_summary_for_period};

    fn date(year: i32, month: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(year, month, day).expect("valid date")
    }

    #[rstest]
    #[case::fiscal_year("FYFinancialStatements_SampleA", Some("FY"), "FY", true)]
    #[case::first_quarter("1QFinancialStatements_SampleA", Some("1Q"), "1Q", true)]
    #[case::quarter("2QFinancialStatements_SampleB", Some("2Q"), "2Q", true)]
    #[case::third_quarter("3QFinancialStatements_SampleC", Some("3Q"), "3Q", true)]
    #[case::fourth_quarter("OtherPeriodFinancialStatements_SampleC", Some("4Q"), "4Q", true)]
    #[case::forecast_is_not_actual("EarnForecastRevision", Some("2Q"), "2Q", false)]
    #[case::fiscal_year_is_not_fourth_quarter(
        "FYFinancialStatements_SampleA",
        Some("FY"),
        "4Q",
        false
    )]
    #[case::fourth_quarter_is_not_fiscal_year(
        "OtherPeriodFinancialStatements_SampleC",
        Some("4Q"),
        "FY",
        false
    )]
    #[case::period_type_mismatch("1QFinancialStatements_SampleA", Some("1Q"), "2Q", false)]
    #[case::old_generic_quarter_prefix("FinancialStatements_SampleB", Some("2Q"), "2Q", false)]
    #[case::missing_document_type("", Some("FY"), "FY", false)]
    fn matches_only_actual_documents_for_the_exact_period(
        #[case] document_type: &str,
        #[case] current_period_type: Option<&str>,
        #[case] target_period_type: &str,
        #[case] expected: bool,
    ) {
        let target_period_type =
            FinancialSummaryPeriodType::parse(target_period_type).expect("supported period");
        let document_type = (!document_type.is_empty()).then_some(document_type);

        assert_eq!(
            is_actual_financial_summary_for_period(
                document_type,
                current_period_type,
                Some(date(2030, 6, 30)),
                target_period_type,
                date(2030, 6, 30),
            ),
            expected,
        );
    }

    #[test]
    fn rejects_different_period_end() {
        assert!(!is_actual_financial_summary_for_period(
            Some("2QFinancialStatements_SampleB"),
            Some("2Q"),
            Some(date(2030, 6, 29)),
            FinancialSummaryPeriodType::SecondQuarter,
            date(2030, 6, 30),
        ));
    }
}
