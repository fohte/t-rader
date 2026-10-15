use chrono::Utc;

use crate::strategy_scope::StrategyScope;

use super::super::error::TriggerUseCaseError;
use super::super::types::{CreateTriggerCommand, Trigger, TriggerKind, UpdateTriggerCommand};
use super::validation::{validate_business_days_only, validate_event_match, validate_template};

pub(super) fn validate_create(
    command: &CreateTriggerCommand,
) -> Result<(Option<String>, Option<String>, String), TriggerUseCaseError> {
    let schedule = command
        .schedule
        .as_deref()
        .map(str::trim)
        .map(str::to_string);
    let hook_slug = command
        .hook_slug
        .as_deref()
        .map(str::trim)
        .map(str::to_string);
    validate_business_days_only(command.business_days_only.unwrap_or(false), &command.kind)?;
    match command.kind {
        TriggerKind::Cron => {
            if schedule.as_deref().is_none_or(str::is_empty) {
                return Err(TriggerUseCaseError::Validation(
                    "schedule is required for kind=cron".into(),
                ));
            }
            if hook_slug.is_some() {
                return Err(TriggerUseCaseError::Validation(
                    "hook_slug must be omitted for kind=cron".into(),
                ));
            }
        }
        TriggerKind::Hook => {
            if hook_slug.as_deref().is_none_or(str::is_empty) {
                return Err(TriggerUseCaseError::Validation(
                    "hook_slug is required for kind=hook".into(),
                ));
            }
            if command.schedule.is_some() {
                return Err(TriggerUseCaseError::Validation(
                    "schedule must be omitted for kind=hook".into(),
                ));
            }
        }
    }
    let prompt_template = validate_template(&command.prompt_template)?;
    Ok((schedule, hook_slug, prompt_template))
}

pub(super) fn apply_update(
    mut trigger: Trigger,
    command: UpdateTriggerCommand,
) -> Result<Trigger, TriggerUseCaseError> {
    if let Some(schedule) = command.schedule {
        if trigger.kind != TriggerKind::Cron {
            return Err(TriggerUseCaseError::Validation(
                "schedule can only be set when kind=cron".into(),
            ));
        }
        let schedule = schedule.trim().to_string();
        if schedule.is_empty() {
            return Err(TriggerUseCaseError::Validation(
                "schedule must not be empty".into(),
            ));
        }
        trigger.schedule = Some(schedule);
    }
    if let Some(hook_slug) = command.hook_slug {
        if trigger.kind != TriggerKind::Hook {
            return Err(TriggerUseCaseError::Validation(
                "hook_slug can only be set when kind=hook".into(),
            ));
        }
        let hook_slug = hook_slug.trim().to_string();
        if hook_slug.is_empty() {
            return Err(TriggerUseCaseError::Validation(
                "hook_slug must not be empty".into(),
            ));
        }
        trigger.hook_slug = Some(hook_slug);
    }
    if let Some(event_match) = command.event_match {
        validate_event_match(event_match.as_ref())?;
        trigger.event_match = event_match;
    }
    if let Some(prompt_template) = command.prompt_template {
        trigger.prompt_template = validate_template(&prompt_template)?;
    }
    if let Some(enabled) = command.enabled {
        trigger.enabled = enabled;
    }
    if let Some(business_days_only) = command.business_days_only {
        validate_business_days_only(business_days_only, &trigger.kind)?;
        trigger.business_days_only = business_days_only;
    }
    if let Some(purpose) = command.purpose {
        trigger.purpose = purpose;
    }
    trigger.updated_at = Utc::now().fixed_offset();
    Ok(trigger)
}

pub(super) fn ensure_scope(
    trigger: &Trigger,
    scope: StrategyScope,
) -> Result<(), TriggerUseCaseError> {
    if trigger.strategy_id == Some(scope.id()) {
        Ok(())
    } else {
        Err(TriggerUseCaseError::NotFound(trigger.trigger_id))
    }
}
