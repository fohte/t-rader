use core_application::{news::NewsUseCases, news_aggregator::NewsAggregator};

use super::{DAILY_TIMEOUT, run_with_ingest_run_log_state};

pub struct NewsAggregation;

impl super::SchedulerJob for NewsAggregation {
    const IDENTIFIER: &'static str = core_application::ingest_status::NEWS_AGGREGATION_JOB;

    fn run(state: crate::state::SchedulerState) -> super::JobFuture {
        Box::pin(async move {
            run_with_ingest_run_log_state(
                state,
                <Self as super::SchedulerJob>::IDENTIFIER,
                DAILY_TIMEOUT,
                |state| async move {
                    aggregate_news(
                        &state.dependencies.news,
                        state.dependencies.news_aggregator.as_ref(),
                    )
                    .await
                },
            )
            .await
        })
    }
}

async fn aggregate_news(
    use_cases: &NewsUseCases,
    aggregator: &dyn NewsAggregator,
) -> Result<core_application::news::AggregationStats, String> {
    let stats = use_cases
        .run_aggregation_cycle(aggregator)
        .await
        .map_err(|error| error.to_string())?;
    tracing::debug!(fetched = stats.fetched, "news aggregation cycle completed");
    Ok(stats)
}
