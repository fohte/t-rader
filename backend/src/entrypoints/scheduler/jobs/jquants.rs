use chrono::Utc;
use graphile_worker::{IntoTaskHandlerResult, TaskHandler, WorkerContext};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use super::{DAILY_TIMEOUT, HOURLY_TIMEOUT, run_with_timeout};
use crate::entrypoints::scheduler::state::SchedulerState;

macro_rules! jquants_job {
    ($name:ident, $identifier:literal, $timeout:ident, $body:expr) => {
        #[derive(Debug, Deserialize, Serialize)]
        pub struct $name;

        impl TaskHandler for $name {
            const IDENTIFIER: &'static str = $identifier;

            async fn run(self, context: WorkerContext) -> impl IntoTaskHandlerResult {
                let Some(state) = context.get_ext::<SchedulerState>() else {
                    return Err("scheduler state is not configured".to_string());
                };

                run_with_timeout(Self::IDENTIFIER, $timeout, async {
                    let state = state.clone();
                    let client = state
                        .jquants_client
                        .clone()
                        .ok_or_else(|| "J-Quants client is not configured".to_string())?;
                    ($body)(state, client).await
                })
                .await
            }
        }
    };
}

jquants_job!(
    StockMasterSync,
    "stock_master_sync",
    DAILY_TIMEOUT,
    |state: SchedulerState, client: Arc<gateway_jquants::JQuantsClient>| async move {
        let stats = crate::services::stock_master_sync::run_sync_cycle(&state.db, client.as_ref())
            .await
            .map_err(|error| error.to_string())?;
        tracing::debug!(
            stocks_upserted = stats.stocks_upserted,
            "stock master sync completed"
        );
        Ok(())
    }
);

jquants_job!(
    ShortSaleReportIngest,
    "short_sale_report_ingest",
    DAILY_TIMEOUT,
    |state: SchedulerState, client: Arc<gateway_jquants::JQuantsClient>| async move {
        let stats = state
            .use_cases
            .short_sale_reports()
            .ingest(client.as_ref(), Utc::now().date_naive())
            .await
            .map_err(|error| error.to_string())?;
        tracing::debug!(?stats, "short sale report ingest completed");
        Ok(())
    }
);

jquants_job!(
    ShortRatioIngest,
    "short_ratio_ingest",
    DAILY_TIMEOUT,
    |state: SchedulerState, client: Arc<gateway_jquants::JQuantsClient>| async move {
        let stats = state
            .use_cases
            .short_ratios()
            .ingest(client.as_ref(), Utc::now().date_naive())
            .await
            .map_err(|error| error.to_string())?;
        tracing::debug!(?stats, "short ratio ingest completed");
        Ok(())
    }
);

jquants_job!(
    MarginIngest,
    "margin_ingest",
    DAILY_TIMEOUT,
    |state: SchedulerState, client: Arc<gateway_jquants::JQuantsClient>| async move {
        let (interest_stats, alert_stats) = state
            .use_cases
            .margins()
            .ingest(client.as_ref(), Utc::now().date_naive())
            .await
            .map_err(|error| error.to_string())?;
        tracing::info!(?interest_stats, ?alert_stats, "margin ingest completed");
        Ok(())
    }
);

jquants_job!(
    FinSummaryIngest,
    "fin_summary_ingest",
    DAILY_TIMEOUT,
    |state: SchedulerState, client: Arc<gateway_jquants::JQuantsClient>| async move {
        let stats = crate::services::fin_summary_ingest::run_ingest_cycle(
            &state.db,
            client.as_ref(),
            Utc::now().date_naive(),
        )
        .await
        .map_err(|error| error.to_string())?;
        tracing::debug!(?stats, "financial summary ingest completed");
        Ok(())
    }
);

jquants_job!(
    EarningsDateIngest,
    "earnings_date_ingest",
    DAILY_TIMEOUT,
    |state: SchedulerState, client: Arc<gateway_jquants::JQuantsClient>| async move {
        let stats =
            crate::services::earnings_date_ingest::run_ingest_cycle(&state.db, client.as_ref())
                .await
                .map_err(|error| error.to_string())?;
        tracing::debug!(?stats, "earnings date ingest completed");
        Ok(())
    }
);

jquants_job!(
    EdinetHoldingsIngest,
    "edinet_holdings_ingest",
    DAILY_TIMEOUT,
    |state: SchedulerState, client: Arc<gateway_jquants::JQuantsClient>| async move {
        crate::services::edinet_holdings::run_all(&state.db, client.as_ref()).await?;
        Ok(())
    }
);

jquants_job!(
    ValuationIngest,
    "valuation_ingest",
    HOURLY_TIMEOUT,
    |state: SchedulerState, client: Arc<gateway_jquants::JQuantsClient>| async move {
        let stats = crate::services::valuation_ingest::run_ingest_cycle(&state.db, client.as_ref())
            .await
            .map_err(|error| error.to_string())?;
        tracing::debug!(?stats, "valuation ingest completed");
        Ok(())
    }
);
