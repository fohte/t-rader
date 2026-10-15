use core_application::{
    ingest_status::NEWS_CONTENT_FETCH_JOB, news_content::NewsContentFetchStats,
};
use graphile_worker::{IntoTaskHandlerResult, TaskHandler, WorkerContext};
use serde::{Deserialize, Serialize};

use super::{require_source, run_with_ingest_run_log_state_and_failure_stats};

#[derive(Debug, Deserialize, Serialize)]
pub struct NewsContentFetch;

impl TaskHandler for NewsContentFetch {
    const IDENTIFIER: &'static str = <Self as super::SchedulerJob>::IDENTIFIER;

    async fn run(self, context: WorkerContext) -> impl IntoTaskHandlerResult {
        super::run_from_worker_context(context, <Self as super::SchedulerJob>::run).await
    }
}

impl super::SchedulerJob for NewsContentFetch {
    const IDENTIFIER: &'static str = NEWS_CONTENT_FETCH_JOB;

    fn run(state: crate::state::SchedulerState) -> super::JobFuture {
        Box::pin(async move {
            run_with_ingest_run_log_state_and_failure_stats(
                state,
                <Self as super::SchedulerJob>::IDENTIFIER,
                crate::scheduler::NEWS_CONTENT_JOB_TIMEOUT,
                |state| async move {
                    let fetcher =
                        require_source(state.dependencies.news_content_fetcher, "Firecrawl source")
                            .map_err(|error| (NewsContentFetchStats::default(), error))?;
                    state
                        .dependencies
                        .news_content
                        .fetch_pending(fetcher.as_ref())
                        .await
                        .map_err(|error| (error.stats, error.message))
                },
            )
            .await
        })
    }
}
