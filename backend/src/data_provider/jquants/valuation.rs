use async_trait::async_trait;
use chrono::NaiveDate;
use core_domain::valuation::Valuation;

use super::JQuantsClient;
use super::response::ValuationRecord;
use crate::data_provider::{DataProviderError, DateRange, ValuationSource, ValuationSourceError};
use crate::models::jquants_plan::JQuantsPlan;

#[async_trait]
impl ValuationSource for JQuantsClient {
    async fn fetch_valuations_by_date(
        &self,
        date: NaiveDate,
    ) -> Result<Vec<Valuation>, ValuationSourceError> {
        JQuantsClient::fetch_valuation_by_date(self, date)
            .await?
            .into_iter()
            .map(valuation_from_record)
            .collect::<Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    /// Valuation は Standard 以上でのみ提供される。
    fn fetchable_range(&self, today: NaiveDate) -> Option<DateRange> {
        match self.manual_plan() {
            Some(JQuantsPlan::Standard | JQuantsPlan::Premium) => {
                self.manual_plan_date_range(today)
            }
            plan => {
                tracing::debug!(
                    ?plan,
                    "valuation は Standard 以上の契約プランが必要なため取得できません"
                );
                None
            }
        }
    }
}

fn valuation_from_record(record: ValuationRecord) -> Result<Valuation, DataProviderError> {
    let date = NaiveDate::parse_from_str(&record.date, "%Y-%m-%d")
        .map_err(|error| DataProviderError::Parse(format!("invalid valuation date: {error}")))?;

    Ok(Valuation {
        code: record.code,
        date,
        eps: record.eps,
        fwd_eps: record.fwd_eps,
        bps: record.bps,
        roe: record.roe,
        fwd_roe: record.fwd_roe,
        per: record.per,
        fwd_per: record.fwd_per,
        pbr: record.pbr,
        mkt_cap: record.mkt_cap,
    })
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;
    use core_domain::valuation::Valuation;
    use rust_decimal::Decimal;
    use serde_json::json;

    use super::*;
    use crate::data_provider::jquants::mock::JQuantsMockServer;

    fn date(year: i32, month: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(year, month, day).expect("valid date")
    }

    fn decimal(value: &str) -> Decimal {
        value.parse().expect("valid decimal")
    }

    #[tokio::test]
    async fn fetches_and_converts_valuation_records() {
        let mock = JQuantsMockServer::start().await;
        mock.valuation()
            .date("2099-01-05")
            .items(vec![json!({
                "Date": "2099-01-05",
                "Code": "ZZZZZ",
                "EPS": 1.25,
                "FwdEPS": 2.5,
                "BPS": 3.75,
                "ROE": 4.0,
                "FwdROE": 5.0,
                "PER": 6.0,
                "FwdPER": 7.0,
                "PBR": 8.0,
                "MktCap": 9.0,
            })])
            .ok()
            .await;
        let client = mock.client().expect("client");

        let valuations = client
            .fetch_valuations_by_date(date(2099, 1, 5))
            .await
            .expect("fetch ok");

        assert_eq!(
            valuations,
            vec![Valuation {
                code: "ZZZZZ".to_string(),
                date: date(2099, 1, 5),
                eps: Some(decimal("1.25")),
                fwd_eps: Some(decimal("2.5")),
                bps: Some(decimal("3.75")),
                roe: Some(decimal("4.0")),
                fwd_roe: Some(decimal("5.0")),
                per: Some(decimal("6.0")),
                fwd_per: Some(decimal("7.0")),
                pbr: Some(decimal("8.0")),
                mkt_cap: Some(decimal("9.0")),
            }],
        );
    }

    #[rstest::rstest]
    #[case::unset(None, None)]
    #[case::free(Some(JQuantsPlan::Free), None)]
    #[case::light(Some(JQuantsPlan::Light), None)]
    #[case::standard(Some(JQuantsPlan::Standard), Some(JQuantsPlan::Standard))]
    #[case::premium(Some(JQuantsPlan::Premium), Some(JQuantsPlan::Premium))]
    fn fetchable_range_requires_standard_or_higher(
        #[case] plan: Option<JQuantsPlan>,
        #[case] fetchable_plan: Option<JQuantsPlan>,
    ) {
        let client =
            JQuantsClient::with_base_url("http://localhost:1", "test-api-key").expect("client");
        let today = date(2099, 1, 5);
        client.set_manual_plan(plan);
        let expected = fetchable_plan.map(|plan| {
            let (from, to) = plan.range(today);
            DateRange { from, to }
        });

        assert_eq!(client.fetchable_range(today), expected);
    }
}
