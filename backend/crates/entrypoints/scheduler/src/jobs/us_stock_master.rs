use core_application::{
    us_stock_master::{UsStockMasterSyncStats, UsStockMasterUseCaseError, UsStockMasterUseCases},
    us_stock_master_source::UsStockMasterSource,
};
use graphile_worker::{IntoTaskHandlerResult, TaskHandler, WorkerContext};
use serde::{Deserialize, Serialize};

use super::{DAILY_TIMEOUT, require_source, run_with_ingest_run_log_state};

#[derive(Debug, Deserialize, Serialize)]
pub struct UsStockMasterIngest;

impl TaskHandler for UsStockMasterIngest {
    const IDENTIFIER: &'static str = core_application::ingest_status::US_STOCK_MASTER_INGEST_JOB;

    async fn run(self, context: WorkerContext) -> impl IntoTaskHandlerResult {
        run_with_ingest_run_log_state(
            context,
            Self::IDENTIFIER,
            DAILY_TIMEOUT,
            |state| async move {
                let source = require_source(
                    state.dependencies.us_stock_master_source,
                    "SEC US stock master source",
                )?;
                sync_us_stock_master(&state.dependencies.us_stock_master, source.as_ref()).await
            },
        )
        .await
    }
}

async fn sync_us_stock_master(
    use_cases: &UsStockMasterUseCases,
    source: &dyn UsStockMasterSource,
) -> Result<UsStockMasterSyncStats, String> {
    let stats = use_cases
        .sync(source)
        .await
        .map_err(|error: UsStockMasterUseCaseError| error.to_string())?;
    tracing::debug!(
        stocks_upserted = stats.stocks_upserted,
        "US stock master sync cycle completed"
    );
    Ok(stats)
}
