use std::collections::HashMap;

use async_trait::async_trait;
use core_application::strategy_task::{
    StrategyTask, StrategyTaskPhase, StrategyTaskRepository, StrategyTaskRepositoryError,
    StrategyTaskStep, StrategyTaskStepStatus, StrategyTaskUpdate, TaskListQuery,
};
use core_application::unit_of_work::UnitOfWorkTransaction;
use sea_orm::ActiveValue::{NotSet, Set, Unchanged};
use sea_orm::sea_query::{Expr, OnConflict};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect, QueryTrait,
};
use uuid::Uuid;

use crate::DatabaseHandle;
use crate::entities::sea_orm_active_enums::{
    StrategyTaskPhase as DbPhase, StrategyTaskStepStatus as DbStepStatus,
};
use crate::entities::{agent_config, strategy, strategy_task, strategy_task_step};
use crate::persistence::persistence_error;
use crate::transaction::transaction_ref as postgres_transaction_ref;

const TASK_LIST_LIMIT: u64 = 50;

#[derive(Clone)]
pub struct PostgresStrategyTaskRepository {
    db: DatabaseHandle,
}

impl PostgresStrategyTaskRepository {
    pub fn new(db: DatabaseHandle) -> Self {
        Self { db }
    }
}

#[async_trait]
impl StrategyTaskRepository for PostgresStrategyTaskRepository {
    async fn strategy_exists(
        &self,
        strategy_id: Uuid,
    ) -> Result<bool, StrategyTaskRepositoryError> {
        strategy::Entity::find_by_id(strategy_id)
            .one(&self.db)
            .await
            .map(|row| row.is_some())
            .map_err(repository_error)
    }

    async fn agent_config_exists(
        &self,
        purpose: &str,
    ) -> Result<bool, StrategyTaskRepositoryError> {
        agent_config::Entity::find()
            .filter(agent_config::Column::Purpose.eq(purpose))
            .one(&self.db)
            .await
            .map(|row| row.is_some())
            .map_err(repository_error)
    }

    async fn insert(
        &self,
        transaction: &UnitOfWorkTransaction,
        task: StrategyTask,
    ) -> Result<(), StrategyTaskRepositoryError> {
        let transaction = transaction_ref(transaction)?;
        let active = strategy_task::ActiveModel {
            task_id: Set(task.task_id),
            strategy_id: Set(task.strategy_id),
            a2a_task_id: Set(task.a2a_task_id),
            source: Set(task.source),
            prompt: Set(task.prompt),
            phase: Set(phase_to_db(task.phase)),
            error_summary: Set(task.error_summary),
            result_text: Set(task.result_text),
            deadline_at: Set(task.deadline_at),
            purpose: Set(task.purpose),
            as_of: Set(task.as_of),
            auto_resumed_at: Set(task.auto_resumed_at),
            created_at: NotSet,
            updated_at: NotSet,
        };
        strategy_task::Entity::insert(active)
            .exec_without_returning(transaction)
            .await
            .map(|_| ())
            .map_err(repository_error)
    }

    async fn update(
        &self,
        transaction: &UnitOfWorkTransaction,
        task: StrategyTaskUpdate,
    ) -> Result<bool, StrategyTaskRepositoryError> {
        let transaction = transaction_ref(transaction)?;
        let active = update_active_model(task);
        active
            .update(transaction)
            .await
            .map(|_| true)
            .map_err(repository_error)
    }

    async fn apply_status_and_steps(
        &self,
        transaction: &UnitOfWorkTransaction,
        task_id: Uuid,
        task_update: Option<StrategyTaskUpdate>,
        steps: Option<serde_json::Value>,
    ) -> Result<bool, StrategyTaskRepositoryError> {
        let transaction = transaction_ref(transaction)?;
        let task_changed = match task_update {
            Some(task_update) => {
                update_active_model(task_update)
                    .update(transaction)
                    .await
                    .map_err(repository_error)?;
                true
            }
            None => false,
        };
        let steps_changed = match steps {
            Some(serde_json::Value::Array(steps)) if !steps.is_empty() => {
                upsert_steps(transaction, task_id, &steps).await?
            }
            _ => false,
        };
        Ok(task_changed || steps_changed)
    }

    async fn find_by_id(
        &self,
        task_id: Uuid,
    ) -> Result<Option<StrategyTask>, StrategyTaskRepositoryError> {
        strategy_task::Entity::find_by_id(task_id)
            .one(&self.db)
            .await
            .map(|row| row.map(task_from_model))
            .map_err(repository_error)
    }

    async fn find_by_a2a_task_id(
        &self,
        a2a_task_id: &str,
    ) -> Result<Option<StrategyTask>, StrategyTaskRepositoryError> {
        strategy_task::Entity::find()
            .filter(strategy_task::Column::A2aTaskId.eq(a2a_task_id))
            .one(&self.db)
            .await
            .map(|row| row.map(task_from_model))
            .map_err(repository_error)
    }

    async fn list(
        &self,
        query: TaskListQuery,
    ) -> Result<Vec<StrategyTask>, StrategyTaskRepositoryError> {
        let mut select = strategy_task::Entity::find();
        if let Some(strategy_id) = query.strategy_id {
            select = select.filter(strategy_task::Column::StrategyId.eq(strategy_id));
        }
        if let Some(purpose) = query.purpose {
            select = select.filter(strategy_task::Column::Purpose.eq(purpose));
        }
        select
            .order_by_desc(strategy_task::Column::CreatedAt)
            .limit(TASK_LIST_LIMIT)
            .all(&self.db)
            .await
            .map(|rows| rows.into_iter().map(task_from_model).collect())
            .map_err(repository_error)
    }

    async fn list_in_flight(&self) -> Result<Vec<StrategyTask>, StrategyTaskRepositoryError> {
        strategy_task::Entity::find()
            .filter(strategy_task::Column::Phase.is_in([DbPhase::Pending, DbPhase::Running]))
            .order_by_asc(strategy_task::Column::CreatedAt)
            .all(&self.db)
            .await
            .map(|rows| rows.into_iter().map(task_from_model).collect())
            .map_err(repository_error)
    }

    async fn list_steps(
        &self,
        task_id: Uuid,
    ) -> Result<Vec<StrategyTaskStep>, StrategyTaskRepositoryError> {
        strategy_task_step::Entity::find()
            .filter(strategy_task_step::Column::TaskId.eq(task_id))
            .order_by_asc(strategy_task_step::Column::Seq)
            .all(&self.db)
            .await
            .map(|rows| rows.into_iter().map(step_from_model).collect())
            .map_err(repository_error)
    }

    async fn claim_resumable(
        &self,
        transaction: &UnitOfWorkTransaction,
        task_id: Uuid,
        now: chrono::DateTime<chrono::FixedOffset>,
        mark_auto_resumed: bool,
    ) -> Result<bool, StrategyTaskRepositoryError> {
        let transaction = transaction_ref(transaction)?;
        let failed_step_task_ids = strategy_task_step::Entity::find()
            .select_only()
            .column(strategy_task_step::Column::TaskId)
            .filter(strategy_task_step::Column::TaskId.eq(task_id))
            .filter(strategy_task_step::Column::Status.eq(DbStepStatus::Failed))
            .into_query();
        let resumable = sea_orm::Condition::any()
            .add(strategy_task::Column::Phase.eq(DbPhase::Failed))
            .add(
                sea_orm::Condition::all()
                    .add(strategy_task::Column::Phase.eq(DbPhase::Completed))
                    .add(strategy_task::Column::TaskId.in_subquery(failed_step_task_ids)),
            );
        let mut update = strategy_task::Entity::update_many()
            .col_expr(strategy_task::Column::Phase, Expr::value(DbPhase::Running))
            .col_expr(strategy_task::Column::UpdatedAt, Expr::value(now));
        let mut filter = sea_orm::Condition::all()
            .add(strategy_task::Column::TaskId.eq(task_id))
            .add(resumable);
        if mark_auto_resumed {
            update = update.col_expr(strategy_task::Column::AutoResumedAt, Expr::value(now));
            filter = filter.add(strategy_task::Column::AutoResumedAt.is_null());
        }
        update
            .filter(filter)
            .exec(transaction)
            .await
            .map(|result| result.rows_affected > 0)
            .map_err(repository_error)
    }
}

fn update_active_model(update: StrategyTaskUpdate) -> strategy_task::ActiveModel {
    strategy_task::ActiveModel {
        task_id: Unchanged(update.task_id),
        a2a_task_id: update.a2a_task_id.map_or(NotSet, Set),
        phase: update.phase.map_or(NotSet, |phase| Set(phase_to_db(phase))),
        error_summary: update.error_summary.map_or(NotSet, Set),
        result_text: update.result_text.map_or(NotSet, Set),
        deadline_at: update.deadline_at.map_or(NotSet, Set),
        auto_resumed_at: update.auto_resumed_at.map_or(NotSet, Set),
        updated_at: Set(update.updated_at),
        strategy_id: NotSet,
        source: NotSet,
        prompt: NotSet,
        purpose: NotSet,
        as_of: NotSet,
        created_at: NotSet,
    }
}

fn phase_to_db(phase: StrategyTaskPhase) -> DbPhase {
    match phase {
        StrategyTaskPhase::Pending => DbPhase::Pending,
        StrategyTaskPhase::Running => DbPhase::Running,
        StrategyTaskPhase::Completed => DbPhase::Completed,
        StrategyTaskPhase::Failed => DbPhase::Failed,
    }
}

fn phase_from_db(phase: DbPhase) -> StrategyTaskPhase {
    match phase {
        DbPhase::Pending => StrategyTaskPhase::Pending,
        DbPhase::Running => StrategyTaskPhase::Running,
        DbPhase::Completed => StrategyTaskPhase::Completed,
        DbPhase::Failed => StrategyTaskPhase::Failed,
    }
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

fn task_from_model(model: strategy_task::Model) -> StrategyTask {
    StrategyTask {
        task_id: model.task_id,
        strategy_id: model.strategy_id,
        a2a_task_id: model.a2a_task_id,
        source: model.source,
        prompt: model.prompt,
        phase: phase_from_db(model.phase),
        error_summary: model.error_summary,
        result_text: model.result_text,
        deadline_at: model.deadline_at,
        purpose: model.purpose,
        as_of: model.as_of,
        auto_resumed_at: model.auto_resumed_at,
        created_at: model.created_at,
        updated_at: model.updated_at,
    }
}

fn step_from_model(model: strategy_task_step::Model) -> StrategyTaskStep {
    StrategyTaskStep {
        execution_step_id: model.execution_step_id,
        task_id: model.task_id,
        phase_key: model.phase_key,
        label: model.label,
        model: model.model,
        status: step_status_from_db(model.status),
        item: model.item,
        item_label: model.item_label,
        output: model.output,
        started_at: model.started_at,
        finished_at: model.finished_at,
        trace_id: model.trace_id,
        span_id: model.span_id,
        error: model.error,
        seq: model.seq,
    }
}

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
    started_at: chrono::DateTime<chrono::FixedOffset>,
    #[serde(default)]
    finished_at: Option<chrono::DateTime<chrono::FixedOffset>>,
    trace_id: String,
    span_id: String,
    #[serde(default)]
    error: Option<String>,
}

async fn upsert_steps(
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
        if let Some(row) = existing.get(&wire.execution_step_id)
            && row.status == step_status_to_db(status)
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
            label: Set(wire.label),
            model: Set(wire.model),
            status: Set(step_status_to_db(status)),
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
        .exec(transaction)
        .await
        .map_err(repository_error)?;
    Ok(true)
}

fn transaction_ref(
    transaction: &UnitOfWorkTransaction,
) -> Result<&sea_orm::DatabaseTransaction, StrategyTaskRepositoryError> {
    postgres_transaction_ref(transaction).ok_or(StrategyTaskRepositoryError::InvalidTransaction)
}

fn repository_error(error: sea_orm::DbErr) -> StrategyTaskRepositoryError {
    StrategyTaskRepositoryError::Database(persistence_error(error))
}
