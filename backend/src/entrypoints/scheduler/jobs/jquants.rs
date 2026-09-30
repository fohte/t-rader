use chrono::Utc;
use graphile_worker::{IntoTaskHandlerResult, TaskHandler, WorkerContext};
use serde::{Deserialize, Serialize};

use super::{DAILY_TIMEOUT, HOURLY_TIMEOUT, run_jquants_job};

#[derive(Debug, Deserialize, Serialize)]
pub struct StockMasterSync;

impl TaskHandler for StockMasterSync {
    const IDENTIFIER: &'static str = "stock_master_sync";

    async fn run(self, context: WorkerContext) -> impl IntoTaskHandlerResult {
        run_jquants_job(
            context,
            Self::IDENTIFIER,
            DAILY_TIMEOUT,
            |state, client| async move {
                let stats =
                    crate::services::stock_master_sync::run_sync_cycle(&state.db, client.as_ref())
                        .await
                        .map_err(|error| error.to_string())?;
                tracing::info!(
                    stocks_upserted = stats.stocks_upserted,
                    "stock master sync completed"
                );
                Ok(())
            },
        )
        .await
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ShortSaleReportIngest;

impl TaskHandler for ShortSaleReportIngest {
    const IDENTIFIER: &'static str = "short_sale_report_ingest";

    async fn run(self, context: WorkerContext) -> impl IntoTaskHandlerResult {
        run_jquants_job(
            context,
            Self::IDENTIFIER,
            DAILY_TIMEOUT,
            |state, client| async move {
                let stats = state
                    .use_cases
                    .short_sale_reports()
                    .ingest(client.as_ref(), Utc::now().date_naive())
                    .await
                    .map_err(|error| error.to_string())?;
                tracing::debug!(?stats, "short sale report ingest completed");
                Ok(())
            },
        )
        .await
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ShortRatioIngest;

impl TaskHandler for ShortRatioIngest {
    const IDENTIFIER: &'static str = "short_ratio_ingest";

    async fn run(self, context: WorkerContext) -> impl IntoTaskHandlerResult {
        run_jquants_job(
            context,
            Self::IDENTIFIER,
            DAILY_TIMEOUT,
            |state, client| async move {
                let stats = state
                    .use_cases
                    .short_ratios()
                    .ingest(client.as_ref(), Utc::now().date_naive())
                    .await
                    .map_err(|error| error.to_string())?;
                tracing::debug!(?stats, "short ratio ingest completed");
                Ok(())
            },
        )
        .await
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct MarginIngest;

impl TaskHandler for MarginIngest {
    const IDENTIFIER: &'static str = "margin_ingest";

    async fn run(self, context: WorkerContext) -> impl IntoTaskHandlerResult {
        run_jquants_job(
            context,
            Self::IDENTIFIER,
            DAILY_TIMEOUT,
            |state, client| async move {
                let (interest_stats, alert_stats) = state
                    .use_cases
                    .margins()
                    .ingest(client.as_ref(), Utc::now().date_naive())
                    .await
                    .map_err(|error| error.to_string())?;
                tracing::info!(?interest_stats, ?alert_stats, "margin ingest completed");
                Ok(())
            },
        )
        .await
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct FinSummaryIngest;

impl TaskHandler for FinSummaryIngest {
    const IDENTIFIER: &'static str = "fin_summary_ingest";

    async fn run(self, context: WorkerContext) -> impl IntoTaskHandlerResult {
        run_jquants_job(
            context,
            Self::IDENTIFIER,
            DAILY_TIMEOUT,
            |state, client| async move {
                let stats = crate::services::fin_summary_ingest::run_ingest_cycle(
                    &state.db,
                    client.as_ref(),
                    Utc::now().date_naive(),
                )
                .await
                .map_err(|error| error.to_string())?;
                tracing::debug!(?stats, "financial summary ingest completed");
                Ok(())
            },
        )
        .await
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct EarningsDateIngest;

impl TaskHandler for EarningsDateIngest {
    const IDENTIFIER: &'static str = "earnings_date_ingest";

    async fn run(self, context: WorkerContext) -> impl IntoTaskHandlerResult {
        run_jquants_job(
            context,
            Self::IDENTIFIER,
            DAILY_TIMEOUT,
            |state, client| async move {
                let stats = crate::services::earnings_date_ingest::run_ingest_cycle(
                    &state.db,
                    client.as_ref(),
                )
                .await
                .map_err(|error| error.to_string())?;
                tracing::debug!(?stats, "earnings date ingest completed");
                Ok(())
            },
        )
        .await
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct EdinetHoldingsIngest;

impl TaskHandler for EdinetHoldingsIngest {
    const IDENTIFIER: &'static str = "edinet_holdings_ingest";

    async fn run(self, context: WorkerContext) -> impl IntoTaskHandlerResult {
        run_jquants_job(
            context,
            Self::IDENTIFIER,
            DAILY_TIMEOUT,
            |state, client| async move {
                crate::services::edinet_holdings::run_all(&state.db, client.as_ref()).await?;
                Ok(())
            },
        )
        .await
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ValuationIngest;

impl TaskHandler for ValuationIngest {
    const IDENTIFIER: &'static str = "valuation_ingest";

    async fn run(self, context: WorkerContext) -> impl IntoTaskHandlerResult {
        run_jquants_job(
            context,
            Self::IDENTIFIER,
            HOURLY_TIMEOUT,
            |state, client| async move {
                let stats =
                    crate::services::valuation_ingest::run_ingest_cycle(&state.db, client.as_ref())
                        .await
                        .map_err(|error| error.to_string())?;
                tracing::debug!(?stats, "valuation ingest completed");
                Ok(())
            },
        )
        .await
    }
}
