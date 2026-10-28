use async_trait::async_trait;
use core_application::strategy_task::{
    StrategyTask, StrategyTaskApplyResult, StrategyTaskOrigin, StrategyTaskPhase,
    StrategyTaskRepository, StrategyTaskRepositoryError, StrategyTaskStep, StrategyTaskUpdate,
    TaskListQuery,
};
use core_application::unit_of_work::UnitOfWorkTransaction;
use sea_orm::ActiveValue::{NotSet, Set, Unchanged};
use sea_orm::sea_query::{Condition, Expr, LockType};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect, QueryTrait,
};
use uuid::Uuid;

use crate::DatabaseHandle;
use crate::entities::sea_orm_active_enums::{
    StrategyTaskPhase as DbPhase, StrategyTaskStepStatus as DbStepStatus,
};
use crate::entities::{
    agent_config, strategy, strategy_task, strategy_task_step, task_execution_state,
};
use crate::persistence::persistence_error;
use crate::transaction::transaction_ref as postgres_transaction_ref;

const TASK_LIST_LIMIT: u64 = 50;

mod steps;

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

    async fn find_agent_graph_by_purpose(
        &self,
        purpose: &str,
    ) -> Result<Option<String>, StrategyTaskRepositoryError> {
        agent_config::Entity::find()
            .filter(agent_config::Column::Purpose.eq(purpose))
            .one(&self.db)
            .await
            .map(|row| row.map(|config| config.agent_graph))
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
            context_kind: Set(task.context_kind),
            context_id: Set(task.context_id),
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
        expected_a2a_task_id: Option<&str>,
        task_update: Option<StrategyTaskUpdate>,
        steps: Option<serde_json::Value>,
    ) -> Result<StrategyTaskApplyResult, StrategyTaskRepositoryError> {
        let transaction = transaction_ref(transaction)?;
        let task_update_attempted = task_update.is_some();
        let task_updated = match task_update {
            Some(task_update) => {
                let update = strategy_task::Entity::update_many()
                    .set(update_active_model(task_update))
                    .filter(strategy_task::Column::TaskId.eq(task_id))
                    .filter(
                        strategy_task::Column::Phase.is_in([DbPhase::Pending, DbPhase::Running]),
                    )
                    .filter(a2a_task_id_condition(expected_a2a_task_id));
                update
                    .exec(transaction)
                    .await
                    .map(|result| result.rows_affected > 0)
                    .map_err(repository_error)?
            }
            None => false,
        };

        let has_steps =
            matches!(&steps, Some(serde_json::Value::Array(steps)) if !steps.is_empty());
        let can_update_steps = if task_updated {
            true
        } else if !task_update_attempted && has_steps {
            let select = strategy_task::Entity::find_by_id(task_id)
                .filter(strategy_task::Column::Phase.is_in([DbPhase::Pending, DbPhase::Running]))
                .lock(LockType::Update)
                .filter(a2a_task_id_condition(expected_a2a_task_id));
            select
                .one(transaction)
                .await
                .map_err(repository_error)?
                .is_some()
        } else {
            false
        };

        let steps_updated = match (can_update_steps, steps) {
            (true, Some(serde_json::Value::Array(steps))) if !steps.is_empty() => {
                steps::upsert_steps(transaction, task_id, &steps).await?
            }
            _ => false,
        };
        Ok(StrategyTaskApplyResult {
            task_updated,
            steps_updated,
        })
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

    async fn find_origin_by_execution_step_id(
        &self,
        execution_step_id: Uuid,
    ) -> Result<Option<StrategyTaskOrigin>, StrategyTaskRepositoryError> {
        let Some(step) = strategy_task_step::Entity::find_by_id(execution_step_id)
            .one(&self.db)
            .await
            .map_err(repository_error)?
        else {
            return Ok(None);
        };
        strategy_task::Entity::find_by_id(step.task_id)
            .one(&self.db)
            .await
            .map(|task| {
                task.map(|task| StrategyTaskOrigin {
                    strategy_id: task.strategy_id,
                    task_id: task.task_id,
                })
            })
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
        if let Some(context) = query.context {
            select = select
                .filter(strategy_task::Column::ContextKind.eq(context.kind))
                .filter(strategy_task::Column::ContextId.eq(context.id));
        }
        select
            .order_by_desc(strategy_task::Column::CreatedAt)
            .limit(TASK_LIST_LIMIT)
            .all(&self.db)
            .await
            .map(|rows| rows.into_iter().map(task_from_model).collect())
            .map_err(repository_error)
    }

    async fn list_model_call_limit_reached_task_ids(
        &self,
        task_ids: &[Uuid],
    ) -> Result<Vec<Uuid>, StrategyTaskRepositoryError> {
        steps::list_model_call_limit_reached_task_ids(&self.db, task_ids).await
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
        steps::list_steps(&self.db, task_id).await
    }

    async fn claim_resumable(
        &self,
        transaction: &UnitOfWorkTransaction,
        task_id: Uuid,
        now: chrono::DateTime<chrono::FixedOffset>,
        mark_auto_resumed: bool,
    ) -> Result<bool, StrategyTaskRepositoryError> {
        let transaction = transaction_ref(transaction)?;
        let unexpired_task_ids = task_execution_state::Entity::find()
            .select_only()
            .column(task_execution_state::Column::TaskId)
            .filter(task_execution_state::Column::DeadlineAt.gt(now))
            .filter(task_execution_state::Column::UsageComplete.eq(true))
            .into_query();
        let failed_step_task_ids = strategy_task_step::Entity::find()
            .select_only()
            .column(strategy_task_step::Column::TaskId)
            .filter(strategy_task_step::Column::Status.eq(DbStepStatus::Failed))
            .into_query();
        let resumable = Condition::any()
            .add(strategy_task::Column::Phase.eq(DbPhase::Failed))
            .add(
                Condition::all()
                    .add(strategy_task::Column::Phase.eq(DbPhase::Completed))
                    .add(strategy_task::Column::TaskId.in_subquery(failed_step_task_ids)),
            );
        let mut filter = Condition::all()
            .add(strategy_task::Column::TaskId.eq(task_id))
            .add(strategy_task::Column::TaskId.in_subquery(unexpired_task_ids))
            .add(resumable);
        let mut update = strategy_task::Entity::update_many()
            .col_expr(strategy_task::Column::Phase, Expr::value(DbPhase::Running))
            .col_expr(
                strategy_task::Column::A2aTaskId,
                Expr::value(None::<String>),
            )
            .col_expr(strategy_task::Column::UpdatedAt, Expr::value(now));
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

fn a2a_task_id_condition(expected_a2a_task_id: Option<&str>) -> sea_orm::sea_query::SimpleExpr {
    match expected_a2a_task_id {
        Some(expected_a2a_task_id) => {
            strategy_task::Column::A2aTaskId.eq(expected_a2a_task_id.to_string())
        }
        None => strategy_task::Column::A2aTaskId.is_null(),
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
        context_kind: NotSet,
        context_id: NotSet,
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
        context_kind: model.context_kind,
        context_id: model.context_id,
        auto_resumed_at: model.auto_resumed_at,
        created_at: model.created_at,
        updated_at: model.updated_at,
    }
}

fn transaction_ref(
    transaction: &UnitOfWorkTransaction,
) -> Result<&sea_orm::DatabaseTransaction, StrategyTaskRepositoryError> {
    postgres_transaction_ref(transaction).ok_or(StrategyTaskRepositoryError::InvalidTransaction)
}

fn repository_error(error: sea_orm::DbErr) -> StrategyTaskRepositoryError {
    StrategyTaskRepositoryError::Database(persistence_error(error))
}

#[cfg(test)]
#[path = "strategy_task/tests.rs"]
mod tests;
