use sea_orm_migration::prelude::*;

pub struct Migration;

#[derive(DeriveIden)]
enum StrategyTask {
    Table,
    TaskId,
}

#[derive(DeriveIden)]
enum TaskExecutionState {
    Table,
    TaskId,
    DeadlineAt,
    ModelCallsUsed,
    ToolCallsUsed,
    SubagentRunsUsed,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum ExecutionReservation {
    Table,
    ReservationId,
    TaskId,
    CallId,
    Resource,
    ResourceKey,
    ScopeKey,
    Amount,
    State,
    CreatedAt,
    UpdatedAt,
}

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20261022_093017_execution_budget"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(TaskExecutionState::Table)
                    .col(
                        ColumnDef::new(TaskExecutionState::TaskId)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(TaskExecutionState::DeadlineAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(TaskExecutionState::ModelCallsUsed)
                            .big_integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(TaskExecutionState::ToolCallsUsed)
                            .big_integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(TaskExecutionState::SubagentRunsUsed)
                            .big_integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(TaskExecutionState::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(TaskExecutionState::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(TaskExecutionState::Table, TaskExecutionState::TaskId)
                            .to(StrategyTask::Table, StrategyTask::TaskId)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .check(Expr::col(TaskExecutionState::ModelCallsUsed).gte(0))
                    .check(Expr::col(TaskExecutionState::ToolCallsUsed).gte(0))
                    .check(Expr::col(TaskExecutionState::SubagentRunsUsed).gte(0))
                    .to_owned(),
            )
            .await?;

        manager
            .get_connection()
            .execute_unprepared(
                "INSERT INTO task_execution_state (task_id, deadline_at, created_at, updated_at) \
                 SELECT task_id, deadline_at, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP \
                 FROM strategy_task",
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(ExecutionReservation::Table)
                    .col(
                        ColumnDef::new(ExecutionReservation::ReservationId)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(ExecutionReservation::TaskId)
                            .uuid()
                            .not_null(),
                    )
                    .col(ColumnDef::new(ExecutionReservation::CallId).string_len(255).not_null())
                    .col(
                        ColumnDef::new(ExecutionReservation::Resource)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ExecutionReservation::ResourceKey)
                            .string_len(128)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ExecutionReservation::ScopeKey)
                            .string_len(128)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ExecutionReservation::Amount)
                            .big_integer()
                            .not_null(),
                    )
                    .col(ColumnDef::new(ExecutionReservation::State).text().not_null())
                    .col(
                        ColumnDef::new(ExecutionReservation::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ExecutionReservation::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .check(Expr::col(ExecutionReservation::Resource).is_in([
                        "model_call",
                        "tool_call",
                        "subagent_run",
                    ]))
                    .check(Expr::col(ExecutionReservation::Amount).gt(0))
                    .check(Expr::col(ExecutionReservation::State).is_in([
                        "reserved",
                        "consumed",
                        "released",
                    ]))
                    .foreign_key(
                        ForeignKey::create()
                            .from(ExecutionReservation::Table, ExecutionReservation::TaskId)
                            .to(TaskExecutionState::Table, TaskExecutionState::TaskId)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .index(
                        Index::create()
                            .unique()
                            .col(ExecutionReservation::TaskId)
                            .col(ExecutionReservation::CallId)
                            .col(ExecutionReservation::ResourceKey)
                            .col(ExecutionReservation::ScopeKey),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_execution_reservation_scope_state")
                    .table(ExecutionReservation::Table)
                    .col(ExecutionReservation::TaskId)
                    .col(ExecutionReservation::Resource)
                    .col(ExecutionReservation::ResourceKey)
                    .col(ExecutionReservation::ScopeKey)
                    .col(ExecutionReservation::State)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(ExecutionReservation::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(TaskExecutionState::Table).to_owned())
            .await?;
        Ok(())
    }
}
