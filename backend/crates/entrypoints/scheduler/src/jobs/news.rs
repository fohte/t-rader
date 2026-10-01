use core_application::{news::NewsUseCases, news_aggregator::NewsAggregator};
use graphile_worker::{IntoTaskHandlerResult, TaskHandler, WorkerContext};
use serde::{Deserialize, Serialize};

use super::{DAILY_TIMEOUT, run_with_ingest_run_log_state};

#[derive(Debug, Deserialize, Serialize)]
pub struct NewsAggregation;

impl TaskHandler for NewsAggregation {
    const IDENTIFIER: &'static str = "news_aggregation";

    async fn run(self, context: WorkerContext) -> impl IntoTaskHandlerResult {
        run_with_ingest_run_log_state(
            context,
            Self::IDENTIFIER,
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
