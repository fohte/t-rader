use chrono::Utc;

use crate::strategy_scope::StrategyScope;

use super::super::error::TriggerUseCaseError;
use super::super::types::{CreateTriggerCommand, Trigger, TriggerKind, UpdateTriggerCommand};
use super::validation::{
    normalize_event_type, validate_business_days_only, validate_event_match, validate_template,
};

pub(super) struct ValidatedTriggerFields {
    pub(super) schedule: Option<String>,
    pub(super) hook_slug: Option<String>,
    pub(super) event_type: Option<String>,
    pub(super) prompt_template: String,
}

pub(super) fn validate_create(
    command: &CreateTriggerCommand,
) -> Result<ValidatedTriggerFields, TriggerUseCaseError> {
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
    let event_type = normalize_event_type(command.event_type.as_deref())?;
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
            if event_type.is_some() {
                return Err(TriggerUseCaseError::Validation(
                    "event_type must be omitted for kind=cron".into(),
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
            if event_type.is_some() {
                return Err(TriggerUseCaseError::Validation(
                    "event_type must be omitted for kind=hook".into(),
                ));
            }
        }
        TriggerKind::Event => {
            if event_type.is_none() {
                return Err(TriggerUseCaseError::Validation(
                    "event_type is required for kind=event".into(),
                ));
            }
            if schedule.is_some() {
                return Err(TriggerUseCaseError::Validation(
                    "schedule must be omitted for kind=event".into(),
                ));
            }
            if hook_slug.is_some() {
                return Err(TriggerUseCaseError::Validation(
                    "hook_slug must be omitted for kind=event".into(),
                ));
            }
        }
    }
    let prompt_template = validate_template(&command.prompt_template)?;
    Ok(ValidatedTriggerFields {
        schedule,
        hook_slug,
        event_type,
        prompt_template,
    })
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
    if let Some(event_type) = command.event_type {
        let event_type = normalize_event_type(event_type.as_deref())?;
        match trigger.kind {
            TriggerKind::Event => {
                if event_type.is_none() {
                    return Err(TriggerUseCaseError::Validation(
                        "event_type is required for kind=event".into(),
                    ));
                }
                trigger.event_type = event_type;
            }
            TriggerKind::Cron | TriggerKind::Hook if event_type.is_some() => {
                return Err(TriggerUseCaseError::Validation(format!(
                    "event_type can only be set when kind=event (kind={})",
                    trigger.kind.as_str(),
                )));
            }
            TriggerKind::Cron | TriggerKind::Hook => {}
        }
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
