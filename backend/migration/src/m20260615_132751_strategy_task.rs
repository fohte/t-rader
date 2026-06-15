use sea_orm_migration::{prelude::*, sea_query::extension::postgres::Type};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(DeriveIden)]
enum Strategy {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum StrategyTask {
    Table,
    TaskId,
    StrategyId,
    KubeopencodeTaskName,
    Source,
    Prompt,
    Phase,
    ErrorSummary,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
struct StrategyTaskPhase;

#[derive(DeriveIden)]
enum StrategyTaskPhaseVariant {
    Pending,
    Running,
    Completed,
    Failed,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_type(
                Type::create()
                    .as_enum(StrategyTaskPhase)
                    .values([
                        StrategyTaskPhaseVariant::Pending,
                        StrategyTaskPhaseVariant::Running,
                        StrategyTaskPhaseVariant::Completed,
                        StrategyTaskPhaseVariant::Failed,
                    ])
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(StrategyTask::Table)
                    .col(
                        ColumnDef::new(StrategyTask::TaskId)
                            .uuid()
                            .not_null()
                            .primary_key()
                            .default(Expr::cust("gen_random_uuid()")),
                    )
                    .col(ColumnDef::new(StrategyTask::StrategyId).uuid().not_null())
                    .col(
                        ColumnDef::new(StrategyTask::KubeopencodeTaskName)
                            .text()
                            .not_null()
                            .unique_key(),
                    )
                    .col(ColumnDef::new(StrategyTask::Source).text().not_null())
                    .col(ColumnDef::new(StrategyTask::Prompt).text().not_null())
                    .col(
                        ColumnDef::new(StrategyTask::Phase)
                            .custom(StrategyTaskPhase)
                            .not_null()
                            .default(Expr::cust("'pending'::strategy_task_phase")),
                    )
                    .col(ColumnDef::new(StrategyTask::ErrorSummary).text())
                    .col(
                        ColumnDef::new(StrategyTask::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(StrategyTask::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(StrategyTask::Table, StrategyTask::StrategyId)
                            .to(Strategy::Table, Strategy::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("strategy_task_strategy_idx")
                    .table(StrategyTask::Table)
                    .col(StrategyTask::StrategyId)
                    .col((StrategyTask::CreatedAt, IndexOrder::Desc))
                    .to_owned(),
            )
            .await?;

        manager
            .get_connection()
            .execute_unprepared(
                "CREATE INDEX strategy_task_phase_idx ON strategy_task (phase) \
                 WHERE phase IN ('pending', 'running')",
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(StrategyTask::Table).to_owned())
            .await?;
        manager
            .drop_type(Type::drop().name(StrategyTaskPhase).to_owned())
            .await?;
        Ok(())
    }
}
