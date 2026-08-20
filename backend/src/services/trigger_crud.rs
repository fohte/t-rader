//! trigger の検証と CRUD (list/create/get/update/delete) の共通 service。
//!
//! REST ハンドラ (`backend/src/handlers/triggers.rs`) はこの service の薄いラッパー。
//! trigger の発火 (`fire_trigger` 等) は `services::triggers` を参照。CRUD と発火は
//! 責務が異なるため別ファイルにしている。

use sea_orm::ActiveModelTrait;
use sea_orm::ActiveValue::{NotSet, Set};
use sea_orm::{
    ColumnTrait, DatabaseConnection, EntityTrait, IntoActiveModel, QueryFilter, QueryOrder,
};
use uuid::Uuid;

use crate::entities::trigger;
use crate::error::AppError;
use crate::models::{CreateTriggerRequest, TriggerKind, UpdateTriggerRequest};

fn validate_template(value: &str) -> Result<String, AppError> {
    let trimmed = value.trim().to_string();
    if trimmed.is_empty() {
        return Err(AppError::Validation(
            "prompt_template must not be empty".into(),
        ));
    }
    Ok(trimmed)
}

fn validate_event_match(event_match: Option<&serde_json::Value>) -> Result<(), AppError> {
    match event_match {
        Some(v) if !v.is_object() && !v.is_null() => Err(AppError::Validation(
            "event_match must be an object or null".into(),
        )),
        _ => Ok(()),
    }
}

fn validate_create(payload: &CreateTriggerRequest) -> Result<(), AppError> {
    validate_event_match(payload.event_match.as_ref())?;
    match payload.kind {
        TriggerKind::Cron => {
            if payload
                .schedule
                .as_deref()
                .map(str::trim)
                .map(str::is_empty)
                .unwrap_or(true)
            {
                return Err(AppError::Validation(
                    "schedule is required for kind=cron".into(),
                ));
            }
            if payload.hook_slug.is_some() {
                return Err(AppError::Validation(
                    "hook_slug must be omitted for kind=cron".into(),
                ));
            }
        }
        TriggerKind::Hook => {
            if payload
                .hook_slug
                .as_deref()
                .map(str::trim)
                .map(str::is_empty)
                .unwrap_or(true)
            {
                return Err(AppError::Validation(
                    "hook_slug is required for kind=hook".into(),
                ));
            }
            if payload.schedule.is_some() {
                return Err(AppError::Validation(
                    "schedule must be omitted for kind=hook".into(),
                ));
            }
        }
    }
    Ok(())
}

async fn find_strategy_or_404(db: &DatabaseConnection, id: Uuid) -> Result<(), AppError> {
    let exists = crate::entities::strategy::Entity::find_by_id(id)
        .one(db)
        .await?
        .is_some();
    if !exists {
        return Err(AppError::NotFound(format!("strategy {id} not found")));
    }
    Ok(())
}

async fn find_trigger_or_404(
    db: &DatabaseConnection,
    trigger_id: Uuid,
) -> Result<trigger::Model, AppError> {
    trigger::Entity::find_by_id(trigger_id)
        .one(db)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("trigger {trigger_id} not found")))
}

/// 戦略の trigger 一覧
pub async fn list_triggers(
    db: &DatabaseConnection,
    strategy_id: Uuid,
    kind: Option<TriggerKind>,
) -> Result<Vec<trigger::Model>, AppError> {
    find_strategy_or_404(db, strategy_id).await?;
    let mut find = trigger::Entity::find().filter(trigger::Column::StrategyId.eq(strategy_id));
    if let Some(kind) = kind {
        find = find.filter(trigger::Column::Kind.eq(kind.as_str()));
    }
    let items = find
        .order_by_asc(trigger::Column::CreatedAt)
        .all(db)
        .await?;
    Ok(items)
}

/// trigger を作成
pub async fn create_trigger(
    db: &DatabaseConnection,
    strategy_id: Uuid,
    payload: CreateTriggerRequest,
) -> Result<trigger::Model, AppError> {
    validate_create(&payload)?;
    let prompt_template = validate_template(&payload.prompt_template)?;
    find_strategy_or_404(db, strategy_id).await?;

    let model = trigger::ActiveModel {
        trigger_id: Set(Uuid::new_v4()),
        strategy_id: Set(strategy_id),
        kind: Set(payload.kind.as_str().to_string()),
        schedule: Set(payload.schedule.map(|s| s.trim().to_string())),
        hook_slug: Set(payload.hook_slug.map(|s| s.trim().to_string())),
        event_match: Set(payload.event_match),
        prompt_template: Set(prompt_template),
        enabled: Set(payload.enabled.unwrap_or(true)),
        last_fired_at: NotSet,
        created_at: NotSet,
        updated_at: NotSet,
    };
    let created = trigger::Entity::insert(model)
        .exec_with_returning(db)
        .await?;
    Ok(created)
}

/// trigger 詳細
pub async fn get_trigger(
    db: &DatabaseConnection,
    trigger_id: Uuid,
) -> Result<trigger::Model, AppError> {
    find_trigger_or_404(db, trigger_id).await
}

/// trigger 更新 (kind / strategy_id は不変)
pub async fn update_trigger(
    db: &DatabaseConnection,
    trigger_id: Uuid,
    payload: UpdateTriggerRequest,
) -> Result<trigger::Model, AppError> {
    let current = find_trigger_or_404(db, trigger_id).await?;
    let mut active = current.clone().into_active_model();

    if let Some(schedule) = payload.schedule {
        if current.kind != TriggerKind::Cron.as_str() {
            return Err(AppError::Validation(
                "schedule can only be set when kind=cron".into(),
            ));
        }
        let trimmed = schedule.trim().to_string();
        if trimmed.is_empty() {
            return Err(AppError::Validation("schedule must not be empty".into()));
        }
        active.schedule = Set(Some(trimmed));
    }
    if let Some(hook_slug) = payload.hook_slug {
        if current.kind != TriggerKind::Hook.as_str() {
            return Err(AppError::Validation(
                "hook_slug can only be set when kind=hook".into(),
            ));
        }
        let trimmed = hook_slug.trim().to_string();
        if trimmed.is_empty() {
            return Err(AppError::Validation("hook_slug must not be empty".into()));
        }
        active.hook_slug = Set(Some(trimmed));
    }
    if let Some(event_match) = payload.event_match {
        validate_event_match(Some(&event_match))?;
        active.event_match = Set(Some(event_match));
    }
    if let Some(prompt_template) = payload.prompt_template {
        active.prompt_template = Set(validate_template(&prompt_template)?);
    }
    if let Some(enabled) = payload.enabled {
        active.enabled = Set(enabled);
    }
    active.updated_at = Set(chrono::Utc::now().fixed_offset());

    let updated = active.update(db).await?;
    Ok(updated)
}

/// trigger 削除
pub async fn delete_trigger(db: &DatabaseConnection, trigger_id: Uuid) -> Result<(), AppError> {
    let result = trigger::Entity::delete_by_id(trigger_id).exec(db).await?;
    if result.rows_affected == 0 {
        return Err(AppError::NotFound(format!(
            "trigger {trigger_id} not found"
        )));
    }
    Ok(())
}
