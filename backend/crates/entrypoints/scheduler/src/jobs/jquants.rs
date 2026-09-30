use chrono::Utc;
use core_application::{
    margin::MarginUseCases, margin_source::MarginSource, short_ratio::ShortRatioUseCases,
    short_sale_report::ShortSaleReportUseCases, short_selling_source::ShortSellingSource,
};
use graphile_worker::{IntoTaskHandlerResult, TaskHandler, WorkerContext};
use serde::{Deserialize, Serialize};

use super::{DAILY_TIMEOUT, run_with_state};

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
                let Some(source) = state.dependencies.short_selling_source else {
                    return Err("J-Quants short-selling source is not configured".to_string());
                };
                ingest_short_ratio(&state.dependencies.short_ratios, source.as_ref()).await
            },
        )
        .await
    }
}

async fn ingest_short_ratio(
    use_cases: &ShortRatioUseCases,
    source: &dyn ShortSellingSource,
) -> Result<(), String> {
    let stats = use_cases
        .ingest(source, Utc::now().date_naive())
        .await
        .map_err(|error| error.to_string())?;
    tracing::debug!(
        days_fetched = stats.days_fetched,
        rows_upserted = stats.rows_upserted,
        "short ratio ingest completed"
    );
    Ok(())
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
                let Some(source) = state.dependencies.short_selling_source else {
                    return Err("J-Quants short-selling source is not configured".to_string());
                };
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
) -> Result<(), String> {
    let stats = use_cases
        .ingest(source, Utc::now().date_naive())
        .await
        .map_err(|error| error.to_string())?;
    tracing::debug!(
        days_fetched = stats.days_fetched,
        rows_upserted = stats.rows_upserted,
        "short sale report ingest completed"
    );
    Ok(())
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
                let Some(source) = state.dependencies.margin_source else {
                    return Err("J-Quants margin source is not configured".to_string());
                };
                ingest_margin(&state.dependencies.margins, source.as_ref()).await
            },
        )
        .await
    }
}

async fn ingest_margin(
    use_cases: &MarginUseCases,
    source: &dyn MarginSource,
) -> Result<(), String> {
    let stats = use_cases
        .ingest(source, Utc::now().date_naive())
        .await
        .map_err(|error| error.to_string())?;
    tracing::info!(?stats, "margin ingest completed");
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use async_trait::async_trait;
    use chrono::{NaiveDate, Utc};
    use core_application::{
        daily_bar_source::DateRange,
        short_ratio::{FakeShortRatioRepository, ShortRatioUseCases},
        short_selling_source::{ShortSellingSource, ShortSellingSourceError},
    };
    use core_domain::{short_ratio::ShortRatio, short_sale_report::ShortSaleReport};
    use rstest::rstest;

    use super::ingest_short_ratio;

    struct FakeShortSellingSource {
        range: DateRange,
        ratios: Vec<ShortRatio>,
    }

    #[async_trait]
    impl ShortSellingSource for FakeShortSellingSource {
        async fn fetch_short_sale_reports(
            &self,
            _disc_date: NaiveDate,
        ) -> Result<Vec<ShortSaleReport>, ShortSellingSourceError> {
            Ok(Vec::new())
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

    #[rstest]
    #[case::repository_accepts_rows(false, Ok(()))]
    #[case::repository_error_reaches_the_worker(true, Err(()))]
    #[tokio::test]
    async fn uses_the_application_result_to_determine_job_success(
        #[case] fail_upsert: bool,
        #[case] expected: Result<(), ()>,
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
        };

        assert_eq!(
            ingest_short_ratio(&use_cases, &source)
                .await
                .map_err(|_| ()),
            expected,
        );
    }
}
