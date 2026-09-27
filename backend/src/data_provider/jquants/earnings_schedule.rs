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
        Some(self.plan_date_range(today))
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

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;
    use serde_json::json;

    use super::*;
    use crate::data_provider::EarningsScheduleSource;
    use crate::data_provider::jquants::mock::JQuantsMockServer;

    #[tokio::test]
    async fn test_fetch_earnings_schedules_converts_scheduled_and_undecided_dates() {
        let mock = JQuantsMockServer::start().await;
        let client = mock.client().expect("client");
        let published_date = NaiveDate::from_ymd_opt(2030, 9, 16).expect("valid date");

        mock.earnings_date()
            .date(&published_date.format("%Y-%m-%d").to_string())
            .items(vec![
                json!({
                    "PubDate": "2030-09-16",
                    "SchDate": "2030-11-10",
                    "FQName": "1Q",
                    "FYE": "1231",
                    "Code": "00001",
                    "CoName": "仮想商事",
                    "CoNameEn": "Example Holdings",
                }),
                json!({
                    "PubDate": "2030-09-16",
                    "SchDate": "",
                    "FQName": "2Q",
                    "FYE": "1231",
                    "Code": "00002",
                    "CoName": "架空工業",
                    "CoNameEn": "Fictional Industries",
                }),
            ])
            .ok()
            .await;

        let schedules = client
            .fetch_earnings_schedules_by_date(published_date)
            .await
            .expect("fetch schedules");

        assert_eq!(
            schedules,
            vec![
                EarningsSchedule {
                    code: "00001".to_string(),
                    fiscal_quarter_name: "1Q".to_string(),
                    published_date,
                    scheduled_date: Some(
                        NaiveDate::from_ymd_opt(2030, 11, 10).expect("valid date")
                    ),
                    fiscal_year_end: "1231".to_string(),
                    company_name: "仮想商事".to_string(),
                    company_name_en: "Example Holdings".to_string(),
                },
                EarningsSchedule {
                    code: "00002".to_string(),
                    fiscal_quarter_name: "2Q".to_string(),
                    published_date,
                    scheduled_date: None,
                    fiscal_year_end: "1231".to_string(),
                    company_name: "架空工業".to_string(),
                    company_name_en: "Fictional Industries".to_string(),
                },
            ],
        );
    }
}
