use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use tracing::info;

use crate::{
    unit_of_work::SharedUnitOfWork,
    web_search::{
        SharedWebSearchClient, WebSearchError, WebSearchResult, WebSearchTimeRange, WebSearchTopic,
    },
};

use super::{
    BootstrapCommand, SharedTavilyCreditBudgetRepository, TavilyCreditBudgetConfig,
    TavilyCreditBudgetDenial, TavilyCreditBudgetReason, TavilyCreditBudgetScope,
};
use helpers::{confirmation_failed, denied, month_start, next_day_start, validate_account_usage};

mod execution;
mod helpers;
mod reservations;
mod settlement;

#[async_trait]
pub trait TavilyCreditBudgetUseCasesPort: Send + Sync {
    async fn bootstrap(&self) -> Result<(), TavilyCreditBudgetUseCasesError>;

    async fn search(
        &self,
        call_id: String,
        request_fingerprint: String,
        query: String,
        topic: Option<WebSearchTopic>,
        time_range: Option<WebSearchTimeRange>,
    ) -> Result<Vec<WebSearchResult>, TavilyCreditBudgetUseCasesError>;

    async fn extract_page(
        &self,
        call_id: String,
        request_fingerprint: String,
        url: String,
    ) -> Result<String, TavilyCreditBudgetUseCasesError>;
}

pub type SharedTavilyCreditBudgetUseCases = Arc<dyn TavilyCreditBudgetUseCasesPort + Send + Sync>;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TavilyCreditBudgetUseCasesError {
    #[error("{0}")]
    Denied(TavilyCreditBudgetDenial),
    #[error("{0}")]
    Provider(WebSearchError),
}

impl TavilyCreditBudgetUseCasesError {
    fn denial(&self) -> TavilyCreditBudgetDenial {
        match self {
            Self::Denied(denial) => denial.clone(),
            Self::Provider(_) => TavilyCreditBudgetDenial::new(
                TavilyCreditBudgetReason::OutcomeUnknown,
                TavilyCreditBudgetScope::Confirmation,
                None,
            ),
        }
    }
}

pub trait TavilyCreditBudgetClock: Send + Sync {
    fn now(&self) -> DateTime<Utc>;
}

#[derive(Debug, Default)]
pub struct SystemTavilyCreditBudgetClock;

impl TavilyCreditBudgetClock for SystemTavilyCreditBudgetClock {
    fn now(&self) -> DateTime<Utc> {
        Utc::now()
    }
}

#[derive(Clone)]
pub struct TavilyCreditBudgetUseCases {
    unit_of_work: SharedUnitOfWork,
    repository: SharedTavilyCreditBudgetRepository,
    provider: Option<SharedWebSearchClient>,
    config: Option<TavilyCreditBudgetConfig>,
    clock: Arc<dyn TavilyCreditBudgetClock>,
}

impl TavilyCreditBudgetUseCases {
    pub fn new(
        unit_of_work: SharedUnitOfWork,
        repository: SharedTavilyCreditBudgetRepository,
        provider: Option<SharedWebSearchClient>,
        config: Option<TavilyCreditBudgetConfig>,
        clock: Arc<dyn TavilyCreditBudgetClock>,
    ) -> Self {
        Self {
            unit_of_work,
            repository,
            provider,
            config,
            clock,
        }
    }

    pub async fn bootstrap(&self) -> Result<(), TavilyCreditBudgetUseCasesError> {
        let Some(config) = self.config.as_ref() else {
            return Err(denied(
                TavilyCreditBudgetReason::ConfigurationUnavailable,
                TavilyCreditBudgetScope::Account,
                None,
            ));
        };
        let Some(provider) = self.provider.as_ref() else {
            return Err(denied(
                TavilyCreditBudgetReason::ConfigurationUnavailable,
                TavilyCreditBudgetScope::Provider,
                None,
            ));
        };
        let usage = provider.account_usage().await.map_err(|_| {
            denied(
                TavilyCreditBudgetReason::UsageConfirmationFailed,
                TavilyCreditBudgetScope::Confirmation,
                None,
            )
        })?;
        validate_account_usage(
            &usage.current_plan,
            usage.plan_credits_used,
            usage.paygo_credits_used,
        )?;
        let monthly_provider_usage = usage.total_credits_used().ok_or_else(confirmation_failed)?;

        let now = self.clock.now();
        let current_day = now.date_naive();
        let daily_blocked_until = next_day_start(now).ok_or_else(confirmation_failed)?;
        let transaction = self
            .unit_of_work
            .begin()
            .await
            .map_err(|_| confirmation_failed())?;
        self.repository
            .initialize(
                &transaction,
                BootstrapCommand {
                    config: config.clone(),
                    monthly_provider_usage,
                    month_start: month_start(current_day).ok_or_else(|| {
                        denied(
                            TavilyCreditBudgetReason::UsageConfirmationFailed,
                            TavilyCreditBudgetScope::Confirmation,
                            None,
                        )
                    })?,
                    day_start: current_day,
                    daily_blocked_until,
                    now,
                },
            )
            .await
            .map_err(|_| {
                denied(
                    TavilyCreditBudgetReason::UsageConfirmationFailed,
                    TavilyCreditBudgetScope::Confirmation,
                    None,
                )
            })?;
        self.unit_of_work
            .commit(transaction)
            .await
            .map_err(|_| confirmation_failed())?;
        info!(
            target: "tavily_credit_budget",
            event = "bootstrapped",
            provider_monthly_usage = %monthly_provider_usage,
            daily_blocked_until = %daily_blocked_until.to_rfc3339(),
            "Tavily account credit budget bootstrapped"
        );
        Ok(())
    }
}
