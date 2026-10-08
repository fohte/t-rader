use serde_json::Value;

use super::super::error::TriggerUseCaseError;
use super::super::types::TriggerKind;

pub(super) fn validate_template(template: &str) -> Result<String, TriggerUseCaseError> {
    let template = template.trim().to_string();
    if template.is_empty() {
        return Err(TriggerUseCaseError::Validation(
            "prompt_template must not be empty".into(),
        ));
    }
    Ok(template)
}

pub(super) fn validate_event_match(event_match: Option<&Value>) -> Result<(), TriggerUseCaseError> {
    match event_match {
        Some(value) if !value.is_object() && !value.is_null() => Err(
            TriggerUseCaseError::Validation("event_match must be an object or null".into()),
        ),
        _ => Ok(()),
    }
}

pub(super) fn validate_business_days_only(
    business_days_only: bool,
    kind: &TriggerKind,
) -> Result<(), TriggerUseCaseError> {
    if business_days_only && *kind != TriggerKind::Cron {
        return Err(TriggerUseCaseError::Validation(
            "business_days_only can only be true when kind=cron".into(),
        ));
    }
    Ok(())
}
