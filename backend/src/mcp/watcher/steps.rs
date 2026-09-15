//! t-rader-agent から届く実行ステップ配列の `strategy_task_step` への upsert。

use std::collections::HashMap;

use chrono::{DateTime, FixedOffset};
use sea_orm::ActiveValue::{NotSet, Set};
use sea_orm::sea_query::OnConflict;
use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter};
use uuid::Uuid;

use crate::entities::sea_orm_active_enums::StrategyTaskStepStatus;
use crate::entities::strategy_task_step;

/// t-rader-agent から届いた実行ステップ配列を `strategy_task_step` へ upsert する。
///
/// `execution_step_id` を主キーとして、既存行と `status`/`output`/`finished_at`/`error` が
/// 全て一致する場合は書き込みをスキップする (`phase_key`/`label`/`model`/`item`/`item_label`/
/// `started_at`/`trace_id`/`span_id` はステップ発行時点で確定し不変のため比較・更新対象外)。
pub(super) async fn upsert_steps<C: ConnectionTrait>(
    db: &C,
    task_id: Uuid,
    steps: &[serde_json::Value],
) -> Result<bool, sea_orm::DbErr> {
    let existing: HashMap<Uuid, strategy_task_step::Model> = strategy_task_step::Entity::find()
        .filter(strategy_task_step::Column::TaskId.eq(task_id))
        .all(db)
        .await?
        .into_iter()
        .map(|row| (row.execution_step_id, row))
        .collect();

    let mut to_upsert = Vec::new();
    for raw in steps {
        let wire: StepWire = match serde_json::from_value(raw.clone()) {
            Ok(wire) => wire,
            Err(err) => {
                tracing::warn!(
                    error = %err,
                    task_id = %task_id,
                    "failed to parse strategy task step; skipping",
                );
                continue;
            }
        };
        let Some(status) = step_status_from_raw(&wire.status) else {
            tracing::warn!(
                status = wire.status,
                task_id = %task_id,
                "unknown strategy task step status; skipping",
            );
            continue;
        };

        if let Some(existing_row) = existing.get(&wire.execution_step_id)
            && existing_row.status == status
            && existing_row.output == wire.output
            && existing_row.finished_at == wire.finished_at
            && existing_row.error == wire.error
        {
            continue;
        }

        to_upsert.push(strategy_task_step::ActiveModel {
            execution_step_id: Set(wire.execution_step_id),
            task_id: Set(task_id),
            phase_key: Set(wire.phase_key),
            label: Set(wire.label),
            model: Set(wire.model),
            status: Set(status),
            item: Set(wire.item),
            item_label: Set(wire.item_label),
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
                    strategy_task_step::Column::Output,
                    strategy_task_step::Column::FinishedAt,
                    strategy_task_step::Column::Error,
                ])
                .to_owned(),
        )
        .exec(db)
        .await?;

    Ok(true)
}

/// t-rader-agent から届く実行ステップの wire JSON 形式。`status` は文字列のまま受け取り、
/// `step_status_from_raw` で手動変換する。
#[derive(Debug, serde::Deserialize)]
struct StepWire {
    execution_step_id: Uuid,
    phase_key: String,
    label: String,
    model: String,
    status: String,
    #[serde(default)]
    item: Option<serde_json::Value>,
    #[serde(default)]
    item_label: Option<String>,
    #[serde(default)]
    output: Option<serde_json::Value>,
    started_at: DateTime<FixedOffset>,
    #[serde(default)]
    finished_at: Option<DateTime<FixedOffset>>,
    trace_id: String,
    span_id: String,
    #[serde(default)]
    error: Option<String>,
}

fn step_status_from_raw(raw: &str) -> Option<StrategyTaskStepStatus> {
    match raw {
        "running" => Some(StrategyTaskStepStatus::Running),
        "completed" => Some(StrategyTaskStepStatus::Completed),
        "failed" => Some(StrategyTaskStepStatus::Failed),
        _ => None,
    }
}
