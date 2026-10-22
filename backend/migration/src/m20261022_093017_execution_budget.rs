use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
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
    UsageComplete,
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
    RequestFingerprint,
    State,
    ResponseJson,
    CreatedAt,
    UpdatedAt,
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
                        ColumnDef::new(TaskExecutionState::UsageComplete)
                            .boolean()
                            .not_null()
                            .default(true),
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
                "INSERT INTO task_execution_state (task_id, deadline_at, usage_complete, created_at, updated_at) \
                 SELECT task_id, deadline_at, false, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP \
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
                    .col(
                        ColumnDef::new(ExecutionReservation::RequestFingerprint)
                            .string_len(64)
                            .not_null(),
                    )
                    .col(ColumnDef::new(ExecutionReservation::State).text().not_null())
                    .col(ColumnDef::new(ExecutionReservation::ResponseJson).json_binary())
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

#[cfg(test)]
mod tests {
    #![allow(
        clippy::expect_used,
        reason = "テスト準備の失敗時に詳細を表示するため"
    )]

    use sea_orm::{ConnectionTrait, Database, TransactionTrait};
    use sea_orm_migration::{MigrationTrait, SchemaManager};

    use super::Migration;

    #[tokio::test]
    async fn migration_marks_existing_task_usage_as_unmeasured() {
        let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
        let database = Database::connect(database_url)
            .await
            .expect("connect to test database");
        let transaction = database.begin().await.expect("begin transaction");
        let schema = format!("execution_budget_legacy_test_{}", std::process::id());
        transaction
            .execute_unprepared(&format!("CREATE SCHEMA {schema}"))
            .await
            .expect("create isolated schema");
        transaction
            .execute_unprepared(&format!("SET LOCAL search_path TO {schema}"))
            .await
            .expect("set isolated schema");
        transaction
            .execute_unprepared(indoc::indoc! {r#"
                CREATE TABLE strategy_task (
                    task_id uuid PRIMARY KEY,
                    deadline_at timestamptz NOT NULL
                )
            "#})
            .await
            .expect("create legacy task table");
        transaction
            .execute_unprepared(indoc::indoc! {r#"
                INSERT INTO strategy_task (task_id, deadline_at) VALUES
                    ('00000000-0000-0000-0000-000000000001', '2030-01-01T00:00:00Z'),
                    ('00000000-0000-0000-0000-000000000002', '2030-01-02T00:00:00Z')
            "#})
            .await
            .expect("insert legacy tasks");

        Migration
            .up(&SchemaManager::new(&transaction))
            .await
            .expect("run execution budget migration");
        let first = transaction
            .query_one_raw(sea_orm::Statement::from_string(
                sea_orm::DatabaseBackend::Postgres,
                "SELECT usage_complete, model_calls_used, tool_calls_used, subagent_runs_used FROM task_execution_state WHERE task_id = '00000000-0000-0000-0000-000000000001'",
            ))
            .await
            .expect("query first migrated task")
            .expect("first migrated task exists");
        let second = transaction
            .query_one_raw(sea_orm::Statement::from_string(
                sea_orm::DatabaseBackend::Postgres,
                "SELECT usage_complete, model_calls_used, tool_calls_used, subagent_runs_used FROM task_execution_state WHERE task_id = '00000000-0000-0000-0000-000000000002'",
            ))
            .await
            .expect("query second migrated task")
            .expect("second migrated task exists");
        let actual = (
            (
                first.try_get::<bool>("", "usage_complete").expect("read usage marker"),
                first.try_get::<i64>("", "model_calls_used").expect("read model counter"),
                first.try_get::<i64>("", "tool_calls_used").expect("read tool counter"),
                first.try_get::<i64>("", "subagent_runs_used").expect("read child counter"),
            ),
            (
                second.try_get::<bool>("", "usage_complete").expect("read usage marker"),
                second.try_get::<i64>("", "model_calls_used").expect("read model counter"),
                second.try_get::<i64>("", "tool_calls_used").expect("read tool counter"),
                second.try_get::<i64>("", "subagent_runs_used").expect("read child counter"),
            ),
        );
        transaction.rollback().await.expect("roll back test schema");

        assert_eq!(actual, ((false, 0, 0, 0), (false, 0, 0, 0)));
    }
}
