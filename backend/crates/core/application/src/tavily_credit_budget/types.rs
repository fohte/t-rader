use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::web_search::WebSearchError;

pub const MAX_MONTHLY_CREDIT_LIMIT: Decimal = Decimal::from_parts(900_000_000, 0, 0, false, 6);
pub const MAX_DAILY_CREDIT_LIMIT: Decimal = Decimal::from_parts(25_000_000, 0, 0, false, 6);
pub const SAFE_SEARCH_RESERVATION: Decimal = Decimal::ONE;
pub const SAFE_EXTRACT_RESERVATION: Decimal = Decimal::ONE;

#[derive(Clone, PartialEq, Eq)]
pub struct TavilyCreditBudgetConfig {
    account_id: String,
    daily_limit: Decimal,
    monthly_limit: Decimal,
}

impl TavilyCreditBudgetConfig {
    pub fn free_tier(account_id: String, paygo_disabled_confirmed: bool) -> Option<Self> {
        Self::new(
            account_id,
            MAX_DAILY_CREDIT_LIMIT,
            MAX_MONTHLY_CREDIT_LIMIT,
            paygo_disabled_confirmed,
        )
    }

    pub fn new(
        account_id: String,
        daily_limit: Decimal,
        monthly_limit: Decimal,
        paygo_disabled_confirmed: bool,
    ) -> Option<Self> {
        let account_id = account_id.trim().to_owned();
        if account_id.is_empty()
            || account_id.len() > 255
            || !account_id
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
            || !paygo_disabled_confirmed
            || daily_limit <= Decimal::ZERO
            || monthly_limit <= Decimal::ZERO
            || daily_limit.scale() > 6
            || monthly_limit.scale() > 6
            || daily_limit > MAX_DAILY_CREDIT_LIMIT
            || monthly_limit > MAX_MONTHLY_CREDIT_LIMIT
            || daily_limit > monthly_limit
        {
            return None;
        }

        Some(Self {
            account_id,
            daily_limit,
            monthly_limit,
        })
    }

    pub fn account_id(&self) -> &str {
        &self.account_id
    }

    pub const fn daily_limit(&self) -> Decimal {
        self.daily_limit
    }

    pub const fn monthly_limit(&self) -> Decimal {
        self.monthly_limit
    }
}

impl std::fmt::Debug for TavilyCreditBudgetConfig {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("TavilyCreditBudgetConfig")
            .field("account_id", &"[configured]")
            .field("daily_limit", &self.daily_limit)
            .field("monthly_limit", &self.monthly_limit)
            .finish()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TavilyCreditBudgetOperation {
    Search,
    Extract,
}

impl TavilyCreditBudgetOperation {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Search => "search",
            Self::Extract => "extract",
        }
    }

    pub const fn reservation(self) -> Decimal {
        match self {
            Self::Search => SAFE_SEARCH_RESERVATION,
            Self::Extract => SAFE_EXTRACT_RESERVATION,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TavilyCreditBudgetScope {
    Daily,
    Monthly,
    Provider,
    Account,
    Confirmation,
}

impl TavilyCreditBudgetScope {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Daily => "daily",
            Self::Monthly => "monthly",
            Self::Provider => "provider",
            Self::Account => "account",
            Self::Confirmation => "confirmation",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TavilyCreditBudgetReason {
    ConfigurationUnavailable,
    BootstrapRequired,
    AccountMismatch,
    ConfigurationChanged,
    AccountPlanNotFree,
    PaygoUsageDetected,
    DailyLimitExceeded,
    MonthlyLimitExceeded,
    PeriodResetUnconfirmed,
    UsageConfirmationFailed,
    ProviderRejected,
    OutcomeUnknown,
    IdempotencyConflict,
}

impl TavilyCreditBudgetReason {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ConfigurationUnavailable => "configuration_unavailable",
            Self::BootstrapRequired => "bootstrap_required",
            Self::AccountMismatch => "account_mismatch",
            Self::ConfigurationChanged => "configuration_changed",
            Self::AccountPlanNotFree => "account_plan_not_free",
            Self::PaygoUsageDetected => "paygo_usage_detected",
            Self::DailyLimitExceeded => "daily_limit_exceeded",
            Self::MonthlyLimitExceeded => "monthly_limit_exceeded",
            Self::PeriodResetUnconfirmed => "period_reset_unconfirmed",
            Self::UsageConfirmationFailed => "usage_confirmation_failed",
            Self::ProviderRejected => "provider_rejected",
            Self::OutcomeUnknown => "outcome_unknown",
            Self::IdempotencyConflict => "idempotency_conflict",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TavilyCreditBudgetDenial {
    pub reason: TavilyCreditBudgetReason,
    pub scope: TavilyCreditBudgetScope,
    pub next_reset: Option<DateTime<Utc>>,
    pub provider_status: Option<u16>,
}

impl TavilyCreditBudgetDenial {
    pub const fn new(
        reason: TavilyCreditBudgetReason,
        scope: TavilyCreditBudgetScope,
        next_reset: Option<DateTime<Utc>>,
    ) -> Self {
        Self {
            reason,
            scope,
            next_reset,
            provider_status: None,
        }
    }

    pub const fn with_provider_status(mut self, status: u16) -> Self {
        self.provider_status = Some(status);
        self
    }

    pub fn instruction(&self) -> String {
        let next_reset = self
            .next_reset
            .map(|value| value.to_rfc3339())
            .unwrap_or_else(|| "null".to_owned());
        format!(
            "Tavily request stopped: reason={}, scope={}, next_reset={next_reset}. Do not retry during this execution. Continue with information already available and state what could not be verified.",
            self.reason.as_str(),
            self.scope.as_str(),
        )
    }
}

impl std::fmt::Display for TavilyCreditBudgetDenial {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.instruction())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum CachedTavilyOutcome {
    SearchResults(Vec<crate::web_search::WebSearchResult>),
    ExtractedPage(String),
    ExtractionFailed(WebSearchError),
    Denied(TavilyCreditBudgetDenial),
}
