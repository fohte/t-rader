use chrono::NaiveDate;

/// 決算発表予定 1 件
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EarningsSchedule {
    pub code: String,
    pub fiscal_quarter_name: String,
    pub published_date: NaiveDate,
    pub scheduled_date: Option<NaiveDate>,
    pub fiscal_year_end: String,
    pub company_name: String,
    pub company_name_en: String,
}
