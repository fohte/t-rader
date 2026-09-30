use chrono::Utc;
use core_application::{
    margin::MarginUseCases, margin_source::MarginSource, short_ratio::ShortRatioUseCases,
    short_sale_report::ShortSaleReportUseCases, short_selling_source::ShortSellingSource,
};
use graphile_worker::{IntoTaskHandlerResult, TaskHandler, WorkerContext};
use serde::{Deserialize, Serialize};

use super::{DAILY_TIMEOUT, require_source, run_with_state};

#[derive(Debug, Deserialize, Serialize)]
pub struct ShortRatioIngest;

impl TaskHandler for ShortRatioIngest {
    const IDENTIFIER: &'static str = "short_ratio_ingest";

    async fn run(self, context: WorkerContext) -> impl IntoTaskHandlerResult {
        run_with_state(
            context,
            Self::IDENTIFIER,
            DAILY_TIMEOUT,
            |state| async move {
                let source = require_source(
                    state.dependencies.short_selling_source,
                    "J-Quants short-selling source",
                )?;
                ingest_short_ratio(&state.dependencies.short_ratios, source.as_ref()).await
            },
        )
        .await
    }
}

async fn ingest_short_ratio(
    use_cases: &ShortRatioUseCases,
    source: &dyn ShortSellingSource,
) -> Result<core_application::short_ratio::ShortRatioIngestStats, String> {
    let stats = use_cases
        .ingest(source, Utc::now().date_naive())
        .await
        .map_err(|error| error.to_string())?;
    tracing::debug!(
        days_fetched = stats.days_fetched,
        rows_upserted = stats.rows_upserted,
        "short ratio ingest completed"
    );
    Ok(stats)
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ShortSaleReportIngest;

impl TaskHandler for ShortSaleReportIngest {
    const IDENTIFIER: &'static str = "short_sale_report_ingest";

    async fn run(self, context: WorkerContext) -> impl IntoTaskHandlerResult {
        run_with_state(
            context,
            Self::IDENTIFIER,
            DAILY_TIMEOUT,
            |state| async move {
                let source = require_source(
                    state.dependencies.short_selling_source,
                    "J-Quants short-selling source",
                )?;
                ingest_short_sale_reports(&state.dependencies.short_sale_reports, source.as_ref())
                    .await
            },
        )
        .await
    }
}

async fn ingest_short_sale_reports(
    use_cases: &ShortSaleReportUseCases,
    source: &dyn ShortSellingSource,
) -> Result<core_application::short_sale_report::ShortSaleReportIngestStats, String> {
    let stats = use_cases
        .ingest(source, Utc::now().date_naive())
        .await
        .map_err(|error| error.to_string())?;
    tracing::debug!(
        days_fetched = stats.days_fetched,
        rows_upserted = stats.rows_upserted,
        "short sale report ingest completed"
    );
    Ok(stats)
}

#[derive(Debug, Deserialize, Serialize)]
pub struct MarginIngest;

impl TaskHandler for MarginIngest {
    const IDENTIFIER: &'static str = "margin_ingest";

    async fn run(self, context: WorkerContext) -> impl IntoTaskHandlerResult {
        run_with_state(
            context,
            Self::IDENTIFIER,
            DAILY_TIMEOUT,
            |state| async move {
                let source =
                    require_source(state.dependencies.margin_source, "J-Quants margin source")?;
                ingest_margin(&state.dependencies.margins, source.as_ref()).await
            },
        )
        .await
    }
}

async fn ingest_margin(
    use_cases: &MarginUseCases,
    source: &dyn MarginSource,
) -> Result<
    (
        core_application::margin::IngestStats,
        core_application::margin::IngestStats,
    ),
    String,
> {
    let stats = use_cases
        .ingest(source, Utc::now().date_naive())
        .await
        .map_err(|error| error.to_string())?;
    tracing::info!(?stats, "margin ingest completed");
    Ok(stats)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use async_trait::async_trait;
    use chrono::{NaiveDate, Utc};
    use core_application::{
        daily_bar_source::DateRange,
        margin::{FakeMarginRepository, IngestStats, MarginUseCases},
        margin_source::{MarginSource, MarginSourceError},
        short_ratio::{FakeShortRatioRepository, ShortRatioIngestStats, ShortRatioUseCases},
        short_sale_report::{
            FakeShortSaleReportRepository, ShortSaleReportIngestStats, ShortSaleReportUseCases,
        },
        short_selling_source::{ShortSellingSource, ShortSellingSourceError},
    };
    use core_domain::{
        margin::{MarginAlertRecord, MarginInterestRecord, PubReason},
        short_ratio::ShortRatio,
        short_sale_report::ShortSaleReport,
    };
    use rstest::rstest;

    use super::{ingest_margin, ingest_short_ratio, ingest_short_sale_reports};

    struct FakeShortSellingSource {
        range: DateRange,
        ratios: Vec<ShortRatio>,
        reports: Vec<ShortSaleReport>,
    }

    #[async_trait]
    impl ShortSellingSource for FakeShortSellingSource {
        async fn fetch_short_sale_reports(
            &self,
            _disc_date: NaiveDate,
        ) -> Result<Vec<ShortSaleReport>, ShortSellingSourceError> {
            Ok(self.reports.clone())
        }

        async fn fetch_short_ratios(
            &self,
            _date: NaiveDate,
        ) -> Result<Vec<ShortRatio>, ShortSellingSourceError> {
            Ok(self.ratios.clone())
        }

        fn fetchable_range(&self, _today: NaiveDate) -> Option<DateRange> {
            Some(self.range.clone())
        }
    }

    struct FakeMarginSource {
        range: DateRange,
        interest: Vec<MarginInterestRecord>,
        alerts: Vec<MarginAlertRecord>,
    }

    #[async_trait]
    impl MarginSource for FakeMarginSource {
        async fn fetch_margin_interest(
            &self,
            _date: NaiveDate,
        ) -> Result<Vec<MarginInterestRecord>, MarginSourceError> {
            Ok(self.interest.clone())
        }

        async fn fetch_margin_alert(
            &self,
            _date: NaiveDate,
        ) -> Result<Vec<MarginAlertRecord>, MarginSourceError> {
            Ok(self.alerts.clone())
        }

        fn fetchable_range(&self, _today: NaiveDate) -> Option<DateRange> {
            Some(self.range.clone())
        }
    }

    fn fake_report(date: NaiveDate) -> ShortSaleReport {
        ShortSaleReport {
            disc_date: date,
            calc_date: date,
            code: "X1234".to_string(),
            ss_name: "架空報告者".to_string(),
            ss_addr: "架空住所".to_string(),
            dic_name: "架空委託者".to_string(),
            dic_addr: "架空住所".to_string(),
            fund_name: "架空ファンド".to_string(),
            short_position_ratio: 1.into(),
            short_position_shares: 1,
            short_position_units: 1,
            prev_report_date: None,
            prev_report_ratio: None,
            notes: String::new(),
        }
    }

    fn fake_margin_interest(date: NaiveDate) -> MarginInterestRecord {
        MarginInterestRecord {
            date,
            code: "X1234".to_string(),
            iss_type: 1,
            shrt_vol: 1,
            long_vol: 2,
            shrt_neg_vol: 0,
            long_neg_vol: 0,
            shrt_std_vol: 1,
            long_std_vol: 2,
            shrt_val: None,
            long_val: None,
            shrt_neg_val: None,
            long_neg_val: None,
            shrt_std_val: None,
            long_std_val: None,
        }
    }

    fn fake_margin_alert(date: NaiveDate) -> MarginAlertRecord {
        MarginAlertRecord {
            pub_date: date,
            code: "X1234".to_string(),
            app_date: date,
            pub_reason: PubReason {
                restricted: false,
                daily_publication: false,
                monitoring: false,
                restricted_by_jsf: false,
                precaution_by_jsf: false,
                unclear_or_sec_on_alert: false,
            },
            shrt_out: 1,
            long_out: 2,
            shrt_out_chg: None,
            long_out_chg: None,
            shrt_out_ratio: None,
            long_out_ratio: None,
            sl_ratio: None,
            shrt_neg_out: 0,
            shrt_std_out: 1,
            long_neg_out: 0,
            long_std_out: 2,
            tse_mrgn_reg_cls: "001".to_string(),
        }
    }

    #[rstest]
    #[case::repository_accepts_rows(false, Ok(ShortRatioIngestStats { days_fetched: 1, rows_upserted: 1 }))]
    #[case::repository_error_reaches_the_worker(true, Err(()))]
    #[tokio::test]
    async fn uses_the_application_result_to_determine_job_success(
        #[case] fail_upsert: bool,
        #[case] expected: Result<ShortRatioIngestStats, ()>,
    ) {
        let today = Utc::now().date_naive();
        let repository = Arc::new(FakeShortRatioRepository::new());
        if fail_upsert {
            repository.fail_upserts_with("repository failure");
        }
        let use_cases = ShortRatioUseCases::new(repository);
        let source = FakeShortSellingSource {
            range: DateRange {
                from: today,
                to: today,
            },
            ratios: vec![ShortRatio {
                date: today,
                sector33_code: "sample-sector".to_string(),
                sell_excluding_short_value: None,
                short_with_restriction_value: None,
                short_without_restriction_value: None,
            }],
            reports: Vec::new(),
        };

        assert_eq!(
            ingest_short_ratio(&use_cases, &source)
                .await
                .map_err(|_| ()),
            expected,
        );
    }

    #[rstest]
    #[case::repository_accepts_rows(false, Ok((
        IngestStats { days_fetched: 1, rows_upserted: 1 },
        IngestStats { days_fetched: 1, rows_upserted: 1 },
    )))]
    #[case::repository_failure_is_handled_by_the_use_case(true, Ok((
        IngestStats { days_fetched: 0, rows_upserted: 0 },
        IngestStats { days_fetched: 1, rows_upserted: 1 },
    )))]
    #[tokio::test]
    async fn margin_job_uses_the_application_result(
        #[case] fail_interest_upsert: bool,
        #[case] expected: Result<(IngestStats, IngestStats), ()>,
    ) {
        let today = Utc::now().date_naive();
        let repository = Arc::new(FakeMarginRepository::new());
        if fail_interest_upsert {
            repository
                .fail_interest_upserts_on
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .insert(today);
        }
        let use_cases = MarginUseCases::new(repository);
        let source = FakeMarginSource {
            range: DateRange {
                from: today,
                to: today,
            },
            interest: vec![fake_margin_interest(today)],
            alerts: vec![fake_margin_alert(today)],
        };

        assert_eq!(
            ingest_margin(&use_cases, &source).await.map_err(|_| ()),
            expected,
        );
    }

    #[rstest]
    #[case::repository_accepts_reports(false, Ok(ShortSaleReportIngestStats { days_fetched: 1, rows_upserted: 1 }))]
    #[case::repository_error_reaches_the_worker(true, Err(()))]
    #[tokio::test]
    async fn short_sale_report_job_uses_the_application_result(
        #[case] fail_upsert: bool,
        #[case] expected: Result<ShortSaleReportIngestStats, ()>,
    ) {
        let today = Utc::now().date_naive();
        let repository = Arc::new(FakeShortSaleReportRepository::new());
        if fail_upsert {
            repository.fail_upserts_with("repository failure");
        }
        let use_cases = ShortSaleReportUseCases::new(repository);
        let source = FakeShortSellingSource {
            range: DateRange {
                from: today,
                to: today,
            },
            ratios: Vec::new(),
            reports: vec![fake_report(today)],
        };

        assert_eq!(
            ingest_short_sale_reports(&use_cases, &source)
                .await
                .map_err(|_| ()),
            expected,
        );
    }
}
