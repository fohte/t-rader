use chrono::Utc;
use core_application::{
    financial_summary::{
        FinancialSummaryIngestStats, FinancialSummaryUseCaseError, FinancialSummaryUseCases,
    },
    financial_summary_source::FinancialSummarySource,
};

use super::{
    DAILY_TIMEOUT, require_source, run_and_refresh_screener_snapshot, run_with_ingest_run_log_state,
};

pub struct FinancialSummaryIngest;

impl super::SchedulerJob for FinancialSummaryIngest {
    const IDENTIFIER: &'static str = core_application::ingest_status::FINANCIAL_SUMMARY_INGEST_JOB;

    fn run(state: crate::state::SchedulerState) -> super::JobFuture {
        Box::pin(async move {
            run_with_ingest_run_log_state(
                state,
                <Self as super::SchedulerJob>::IDENTIFIER,
                DAILY_TIMEOUT,
                |state| async move {
                    let source = require_source(
                        state.dependencies.financial_summary_source,
                        "J-Quants financial summary source",
                    )?;
                    run_and_refresh_screener_snapshot(
                        &state.dependencies.screener_snapshot,
                        ingest_financial_summaries(
                            &state.dependencies.financial_summaries,
                            source.as_ref(),
                        ),
                    )
                    .await
                },
            )
            .await
        })
    }
}

async fn ingest_financial_summaries(
    use_cases: &FinancialSummaryUseCases,
    source: &dyn FinancialSummarySource,
) -> Result<FinancialSummaryIngestStats, String> {
    let stats = use_cases
        .run_ingest_cycle(source, Utc::now().date_naive())
        .await
        .map_err(|error: FinancialSummaryUseCaseError| error.to_string())?;
    tracing::debug!(
        days_attempted = stats.days_attempted,
        upserted = stats.upserted,
        "financial summary ingest cycle completed"
    );
    Ok(stats)
}
