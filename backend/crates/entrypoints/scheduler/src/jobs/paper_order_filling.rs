use graphile_worker::{IntoTaskHandlerResult, TaskHandler, WorkerContext};
use serde::{Deserialize, Serialize};

use super::{DAILY_TIMEOUT, run_with_ingest_run_log_state};

#[derive(Debug, Deserialize, Serialize)]
pub struct PaperOrderFilling;

impl TaskHandler for PaperOrderFilling {
    const IDENTIFIER: &'static str = core_application::ingest_status::PAPER_ORDER_FILLING_JOB;

    async fn run(self, context: WorkerContext) -> impl IntoTaskHandlerResult {
        run_with_ingest_run_log_state(
            context,
            Self::IDENTIFIER,
            DAILY_TIMEOUT,
            |state| async move {
                let stats = state
                    .dependencies
                    .paper_trades
                    .fill_pending_orders(chrono::Utc::now().fixed_offset())
                    .await
                    .map_err(|error| error.to_string())?;
                tracing::debug!(
                    filled = stats.filled,
                    rejected = stats.rejected,
                    "paper order filling completed",
                );
                Ok(stats)
            },
        )
        .await
    }
}
