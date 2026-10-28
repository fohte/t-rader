use std::collections::HashMap;

use core_application::strategy_task::{
    StrategyTaskRepositoryError, StrategyTaskStep, StrategyTaskStepKind, StrategyTaskStepStatus,
};
use sea_orm::ActiveValue::{NotSet, Set};
use sea_orm::sea_query::OnConflict;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect};
use uuid::Uuid;

use crate::DatabaseHandle;
use crate::entities::sea_orm_active_enums::StrategyTaskStepStatus as DbStepStatus;
use crate::entities::strategy_task_step;

use super::repository_error;

pub(super) async fn list_model_call_limit_reached_task_ids(
    db: &DatabaseHandle,
    task_ids: &[Uuid],
) -> Result<Vec<Uuid>, StrategyTaskRepositoryError> {
    if task_ids.is_empty() {
        return Ok(Vec::new());
    }
    strategy_task_step::Entity::find()
        .select_only()
        .column(strategy_task_step::Column::TaskId)
        .filter(strategy_task_step::Column::TaskId.is_in(task_ids.to_vec()))
        .filter(strategy_task_step::Column::ModelCallLimitReached.eq(true))
        .distinct()
        .into_tuple::<Uuid>()
        .all(db)
        .await
        .map_err(repository_error)
}

pub(super) async fn list_steps(
    db: &DatabaseHandle,
    task_id: Uuid,
) -> Result<Vec<StrategyTaskStep>, StrategyTaskRepositoryError> {
    strategy_task_step::Entity::find()
        .filter(strategy_task_step::Column::TaskId.eq(task_id))
        .order_by_asc(strategy_task_step::Column::Seq)
        .all(db)
        .await
        .map(|rows| rows.into_iter().map(step_from_model).collect())
        .map_err(repository_error)
}

fn step_status_from_raw(status: &str) -> Option<StrategyTaskStepStatus> {
    match status {
        "running" => Some(StrategyTaskStepStatus::Running),
        "completed" => Some(StrategyTaskStepStatus::Completed),
        "failed" => Some(StrategyTaskStepStatus::Failed),
        _ => None,
    }
}

fn step_status_to_db(status: StrategyTaskStepStatus) -> DbStepStatus {
    match status {
        StrategyTaskStepStatus::Running => DbStepStatus::Running,
        StrategyTaskStepStatus::Completed => DbStepStatus::Completed,
        StrategyTaskStepStatus::Failed => DbStepStatus::Failed,
    }
}

fn step_status_from_db(status: DbStepStatus) -> StrategyTaskStepStatus {
    match status {
        DbStepStatus::Running => StrategyTaskStepStatus::Running,
        DbStepStatus::Completed => StrategyTaskStepStatus::Completed,
        DbStepStatus::Failed => StrategyTaskStepStatus::Failed,
    }
}

fn step_from_model(model: strategy_task_step::Model) -> StrategyTaskStep {
    let kind = StrategyTaskStepKind::parse(&model.kind).unwrap_or_else(|| {
        tracing::warn!(
            execution_step_id = %model.execution_step_id,
            kind = %model.kind,
            "unknown strategy task step kind; reading as LLM step"
        );
        StrategyTaskStepKind::Llm
    });
    StrategyTaskStep {
        execution_step_id: model.execution_step_id,
        task_id: model.task_id,
        phase_key: model.phase_key,
        kind,
        label: model.label,
        model: model.model,
        status: step_status_from_db(model.status),
        item: model.item,
        item_key: model.item_key,
        item_label: model.item_label,
        model_call_limit_reached: model.model_call_limit_reached,
        input_ref: model.input_ref,
        output_ref: model.output_ref,
        output: model.output,
        started_at: model.started_at,
        finished_at: model.finished_at,
        trace_id: model.trace_id,
        span_id: model.span_id,
        error: model.error,
        seq: model.seq,
    }
}

#[derive(Debug, PartialEq, serde::Deserialize)]
struct StepWire {
    execution_step_id: Uuid,
    phase_key: String,
    #[serde(default = "default_step_kind")]
    kind: String,
    label: String,
    #[serde(default)]
    model: Option<String>,
    status: String,
    #[serde(default)]
    item: Option<serde_json::Value>,
    #[serde(default)]
    item_key: Option<serde_json::Value>,
    #[serde(default)]
    item_label: Option<String>,
    #[serde(default)]
    input_ref: Option<serde_json::Value>,
    #[serde(default)]
    output_ref: Option<serde_json::Value>,
    #[serde(default)]
    output: Option<serde_json::Value>,
    started_at: chrono::DateTime<chrono::FixedOffset>,
    #[serde(default)]
    finished_at: Option<chrono::DateTime<chrono::FixedOffset>>,
    trace_id: String,
    span_id: String,
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    model_call_limit_reached: bool,
}

fn default_step_kind() -> String {
    StrategyTaskStepKind::Llm.as_str().to_owned()
}

pub(super) async fn upsert_steps(
    transaction: &sea_orm::DatabaseTransaction,
    task_id: Uuid,
    steps: &[serde_json::Value],
) -> Result<bool, StrategyTaskRepositoryError> {
    let existing: HashMap<Uuid, strategy_task_step::Model> = strategy_task_step::Entity::find()
        .filter(strategy_task_step::Column::TaskId.eq(task_id))
        .all(transaction)
        .await
        .map_err(repository_error)?
        .into_iter()
        .map(|row| (row.execution_step_id, row))
        .collect();

    let mut to_upsert = Vec::new();
    for raw in steps {
        let wire: StepWire = match serde_json::from_value(raw.clone()) {
            Ok(wire) => wire,
            Err(error) => {
                tracing::warn!(error = %error, task_id = %task_id, "failed to parse strategy task step; skipping");
                continue;
            }
        };
        let Some(status) = step_status_from_raw(&wire.status) else {
            tracing::warn!(status = wire.status, task_id = %task_id, "unknown strategy task step status; skipping");
            continue;
        };
        let Some(kind) = StrategyTaskStepKind::parse(&wire.kind) else {
            tracing::warn!(kind = wire.kind, task_id = %task_id, "unknown strategy task step kind; skipping");
            continue;
        };
        // agent は false の印を省略するため、後続送信でも既存の印を保持する。
        let model_call_limit_reached = wire.model_call_limit_reached
            || existing
                .get(&wire.execution_step_id)
                .is_some_and(|row| row.model_call_limit_reached);
        if let Some(row) = existing.get(&wire.execution_step_id)
            && row.kind == kind.as_str()
            && row.model == wire.model
            && row.model_call_limit_reached == model_call_limit_reached
            && row.status == step_status_to_db(status)
            && row.item_key == wire.item_key
            && row.input_ref == wire.input_ref
            && row.output_ref == wire.output_ref
            && row.output == wire.output
            && row.finished_at == wire.finished_at
            && row.error == wire.error
        {
            continue;
        }
        to_upsert.push(strategy_task_step::ActiveModel {
            execution_step_id: Set(wire.execution_step_id),
            task_id: Set(task_id),
            phase_key: Set(wire.phase_key),
            kind: Set(kind.as_str().to_owned()),
            label: Set(wire.label),
            model: Set(wire.model),
            model_call_limit_reached: Set(model_call_limit_reached),
            status: Set(step_status_to_db(status)),
            item: Set(wire.item),
            item_key: Set(wire.item_key),
            item_label: Set(wire.item_label),
            input_ref: Set(wire.input_ref),
            output_ref: Set(wire.output_ref),
            output: Set(wire.output),
            started_at: Set(wire.started_at),
            finished_at: Set(wire.finished_at),
            trace_id: Set(wire.trace_id),
            span_id: Set(wire.span_id),
            error: Set(wire.error),
            seq: NotSet,
        });
    }
    if to_upsert.is_empty() {
        return Ok(false);
    }
    strategy_task_step::Entity::insert_many(to_upsert)
        .on_conflict(
            OnConflict::column(strategy_task_step::Column::ExecutionStepId)
                .update_columns([
                    strategy_task_step::Column::Status,
                    strategy_task_step::Column::ModelCallLimitReached,
                    strategy_task_step::Column::ItemKey,
                    strategy_task_step::Column::InputRef,
                    strategy_task_step::Column::OutputRef,
                    strategy_task_step::Column::Output,
                    strategy_task_step::Column::FinishedAt,
                    strategy_task_step::Column::Error,
                ])
                .to_owned(),
        )
        .exec(transaction)
        .await
        .map_err(repository_error)?;
    Ok(true)
}

#[cfg(test)]
mod tests;
