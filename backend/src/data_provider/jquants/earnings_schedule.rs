use async_trait::async_trait;
use chrono::NaiveDate;
use core_domain::earnings_schedule::EarningsSchedule;

use super::JQuantsClient;
use super::response::EarningsDateRecord;
use crate::data_provider::{
    DataProviderError, DateRange, EarningsScheduleSource, EarningsScheduleSourceError,
};

#[async_trait]
impl EarningsScheduleSource for JQuantsClient {
    async fn fetch_earnings_schedules_by_date(
        &self,
        published_date: NaiveDate,
    ) -> Result<Vec<EarningsSchedule>, EarningsScheduleSourceError> {
        let records = JQuantsClient::fetch_earnings_date_by_date(self, published_date).await?;
        Ok(records
            .into_iter()
            .map(earnings_schedule_from_api)
            .collect::<Result<Vec<_>, _>>()?)
    }

    fn fetchable_range(&self, today: NaiveDate) -> Option<DateRange> {
        self.manual_plan_date_range(today)
    }
}

fn earnings_schedule_from_api(
    record: EarningsDateRecord,
) -> Result<EarningsSchedule, DataProviderError> {
    let published_date = parse_date(&record.pub_date, "PubDate")?;
    let scheduled_date = if record.sch_date.is_empty() {
        None
    } else {
        Some(parse_date(&record.sch_date, "SchDate")?)
    };

    Ok(EarningsSchedule {
        code: record.code,
        fiscal_quarter_name: record.fq_name,
        published_date,
        scheduled_date,
        fiscal_year_end: record.fye,
        company_name: record.co_name,
        company_name_en: record.co_name_en,
    })
}

fn parse_date(value: &str, field_name: &str) -> Result<NaiveDate, DataProviderError> {
    NaiveDate::parse_from_str(value, "%Y-%m-%d")
        .map_err(|error| DataProviderError::Parse(format!("invalid {field_name}: {error}")))
}
