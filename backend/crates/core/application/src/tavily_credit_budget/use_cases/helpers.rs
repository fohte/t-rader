use chrono::{DateTime, Datelike, NaiveDate, TimeZone, Utc};
use rust_decimal::Decimal;
use serde_json::Value;
use tracing::{info, warn};

use super::super::types::CachedTavilyOutcome;
use super::super::{
    ReservationClaim, TavilyCreditBudgetConfig, TavilyCreditBudgetDenial,
    TavilyCreditBudgetOperation, TavilyCreditBudgetReason, TavilyCreditBudgetScope,
    TavilyRetrieved,
};
use super::TavilyCreditBudgetUseCasesError;
use crate::web_search::WebSearchResult;

const FREE_PLAN_NAME: &str = "Researcher";
pub(super) fn replay_claim(
    outcome: Value,
    operation: TavilyCreditBudgetOperation,
) -> Result<ReservationClaim, TavilyCreditBudgetUseCasesError> {
    let outcome = decode_outcome(outcome)?;
    match (operation, outcome) {
        (_, CachedTavilyOutcome::Denied(denial)) => {
            Err(TavilyCreditBudgetUseCasesError::Denied(denial))
        }
        (TavilyCreditBudgetOperation::Search, outcome @ CachedTavilyOutcome::SearchResults(_))
        | (
            TavilyCreditBudgetOperation::Search,
            outcome @ CachedTavilyOutcome::SearchResultsWithFetchedAt { .. },
        )
        | (TavilyCreditBudgetOperation::Extract, outcome @ CachedTavilyOutcome::ExtractedPage(_)) => {
            Ok(ReservationClaim::Replay(encode_outcome(outcome)?))
        }
        (
            TavilyCreditBudgetOperation::Extract,
            outcome @ CachedTavilyOutcome::ExtractedPageWithFetchedAt { .. },
        ) => Ok(ReservationClaim::Replay(encode_outcome(outcome)?)),
        (TavilyCreditBudgetOperation::Extract, CachedTavilyOutcome::ExtractionFailed(error)) => {
            Err(TavilyCreditBudgetUseCasesError::Provider(error))
        }
        _ => Err(idempotency_conflict()),
    }
}

pub(super) fn replay_search(
    outcome: Value,
) -> Result<TavilyRetrieved<Vec<WebSearchResult>>, TavilyCreditBudgetUseCasesError> {
    match decode_outcome(outcome)? {
        CachedTavilyOutcome::SearchResults(results) => Ok(TavilyRetrieved {
            value: results,
            fetched_at: None,
        }),
        CachedTavilyOutcome::SearchResultsWithFetchedAt {
            results,
            fetched_at,
        } => Ok(TavilyRetrieved {
            value: results,
            fetched_at: Some(fetched_at),
        }),
        CachedTavilyOutcome::Denied(denial) => Err(TavilyCreditBudgetUseCasesError::Denied(denial)),
        CachedTavilyOutcome::ExtractionFailed(error) => {
            Err(TavilyCreditBudgetUseCasesError::Provider(error))
        }
        CachedTavilyOutcome::ExtractedPage(_)
        | CachedTavilyOutcome::ExtractedPageWithFetchedAt { .. } => Err(idempotency_conflict()),
    }
}

pub(super) fn replay_extract(
    outcome: Value,
) -> Result<TavilyRetrieved<String>, TavilyCreditBudgetUseCasesError> {
    match decode_outcome(outcome)? {
        CachedTavilyOutcome::ExtractedPage(content) => Ok(TavilyRetrieved {
            value: content,
            fetched_at: None,
        }),
        CachedTavilyOutcome::ExtractedPageWithFetchedAt {
            content,
            fetched_at,
        } => Ok(TavilyRetrieved {
            value: content,
            fetched_at: Some(fetched_at),
        }),
        CachedTavilyOutcome::Denied(denial) => Err(TavilyCreditBudgetUseCasesError::Denied(denial)),
        CachedTavilyOutcome::ExtractionFailed(error) => {
            Err(TavilyCreditBudgetUseCasesError::Provider(error))
        }
        CachedTavilyOutcome::SearchResults(_)
        | CachedTavilyOutcome::SearchResultsWithFetchedAt { .. } => Err(idempotency_conflict()),
    }
}

pub(super) fn decode_outcome(
    value: Value,
) -> Result<CachedTavilyOutcome, TavilyCreditBudgetUseCasesError> {
    serde_json::from_value(value).map_err(|_| confirmation_failed())
}

pub(super) fn encode_outcome(
    outcome: CachedTavilyOutcome,
) -> Result<Value, TavilyCreditBudgetUseCasesError> {
    serde_json::to_value(outcome).map_err(|_| confirmation_failed())
}

pub(super) fn validate_account_usage(
    current_plan: &str,
    plan_credits_used: Decimal,
    paygo_credits_used: Decimal,
) -> Result<(), TavilyCreditBudgetUseCasesError> {
    if plan_credits_used < Decimal::ZERO
        || paygo_credits_used < Decimal::ZERO
        || plan_credits_used.scale() > 6
        || paygo_credits_used.scale() > 6
    {
        return Err(confirmation_failed());
    }
    if current_plan != FREE_PLAN_NAME {
        return Err(denied(
            TavilyCreditBudgetReason::AccountPlanNotFree,
            TavilyCreditBudgetScope::Account,
            None,
        ));
    }
    if paygo_credits_used > Decimal::ZERO {
        return Err(denied(
            TavilyCreditBudgetReason::PaygoUsageDetected,
            TavilyCreditBudgetScope::Account,
            None,
        ));
    }
    Ok(())
}

pub(super) fn valid_usage(actual: Decimal, reserved: Decimal) -> bool {
    actual >= Decimal::ZERO && actual <= reserved && actual.scale() <= 6
}

pub(super) fn valid_exact_usage(actual: Decimal, reserved: Decimal) -> bool {
    actual == reserved && valid_usage(actual, reserved)
}

pub(super) fn month_start(day: NaiveDate) -> Option<NaiveDate> {
    day.with_day(1)
}

pub(super) fn next_day_start(now: DateTime<Utc>) -> Option<DateTime<Utc>> {
    let tomorrow = now.date_naive().succ_opt()?;
    let midnight = tomorrow.and_hms_opt(0, 0, 0)?;
    Some(Utc.from_utc_datetime(&midnight))
}

pub(super) fn denied(
    reason: TavilyCreditBudgetReason,
    scope: TavilyCreditBudgetScope,
    next_reset: Option<DateTime<Utc>>,
) -> TavilyCreditBudgetUseCasesError {
    TavilyCreditBudgetUseCasesError::Denied(TavilyCreditBudgetDenial::new(
        reason, scope, next_reset,
    ))
}

pub(super) fn confirmation_failed() -> TavilyCreditBudgetUseCasesError {
    denied(
        TavilyCreditBudgetReason::UsageConfirmationFailed,
        TavilyCreditBudgetScope::Confirmation,
        None,
    )
}

pub(super) fn configuration_unavailable() -> TavilyCreditBudgetUseCasesError {
    denied(
        TavilyCreditBudgetReason::ConfigurationUnavailable,
        TavilyCreditBudgetScope::Account,
        None,
    )
}

pub(super) fn provider_configuration_unavailable() -> TavilyCreditBudgetUseCasesError {
    denied(
        TavilyCreditBudgetReason::ConfigurationUnavailable,
        TavilyCreditBudgetScope::Provider,
        None,
    )
}

pub(super) fn outcome_unknown() -> TavilyCreditBudgetUseCasesError {
    denied(
        TavilyCreditBudgetReason::OutcomeUnknown,
        TavilyCreditBudgetScope::Confirmation,
        None,
    )
}

pub(super) fn idempotency_conflict() -> TavilyCreditBudgetUseCasesError {
    denied(
        TavilyCreditBudgetReason::IdempotencyConflict,
        TavilyCreditBudgetScope::Account,
        None,
    )
}

pub(super) fn log_reconciliation(
    provider_monthly_usage: Decimal,
    daily_used: Decimal,
    monthly_used: Decimal,
    config: &TavilyCreditBudgetConfig,
) {
    info!(
        target: "tavily_credit_budget",
        event = "usage_reconciled",
        provider_monthly_usage = %provider_monthly_usage,
        daily_used = %daily_used,
        daily_limit = %config.daily_limit(),
        monthly_used = %monthly_used,
        monthly_limit = %config.monthly_limit(),
        "Tavily account usage reconciled"
    );
}

pub(super) fn log_denial(
    denial: &TavilyCreditBudgetDenial,
    operation: TavilyCreditBudgetOperation,
    daily_used: Option<Decimal>,
    monthly_used: Option<Decimal>,
) {
    warn!(
        target: "tavily_credit_budget",
        event = "reservation_denied",
        operation = operation.as_str(),
        reason = denial.reason.as_str(),
        scope = denial.scope.as_str(),
        next_reset = %denial.next_reset.map(|value| value.to_rfc3339()).unwrap_or_else(|| "null".to_owned()),
        provider_status = denial.provider_status,
        daily_used = %daily_used.map(|value| value.to_string()).unwrap_or_else(|| "null".to_owned()),
        monthly_used = %monthly_used.map(|value| value.to_string()).unwrap_or_else(|| "null".to_owned()),
        "Tavily account credit reservation denied"
    );
}
