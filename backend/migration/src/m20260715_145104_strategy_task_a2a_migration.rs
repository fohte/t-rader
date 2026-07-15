use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(DeriveIden)]
enum StrategyTask {
    Table,
    KubeopencodeTaskName,
    A2aTaskId,
    ResultText,
    DeadlineAt,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(StrategyTask::Table)
                    .rename_column(StrategyTask::KubeopencodeTaskName, StrategyTask::A2aTaskId)
                    .to_owned(),
            )
            .await?;
        manager
            .get_connection()
            .execute_unprepared("ALTER TABLE strategy_task ALTER COLUMN a2a_task_id DROP NOT NULL")
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(StrategyTask::Table)
                    .add_column(ColumnDef::new(StrategyTask::ResultText).text())
                    .to_owned(),
            )
            .await?;

        // deadline_at は NOT NULL のため、既存行向けに一旦 DEFAULT 付きで追加してから
        // DEFAULT を外す。以降の INSERT では明示指定を必須にする。
        manager
            .get_connection()
            .execute_unprepared(
                "ALTER TABLE strategy_task ADD COLUMN deadline_at timestamptz NOT NULL DEFAULT now()",
            )
            .await?;
        manager
            .get_connection()
            .execute_unprepared("ALTER TABLE strategy_task ALTER COLUMN deadline_at DROP DEFAULT")
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(StrategyTask::Table)
                    .drop_column(StrategyTask::DeadlineAt)
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(StrategyTask::Table)
                    .drop_column(StrategyTask::ResultText)
                    .to_owned(),
            )
            .await?;
        manager
            .get_connection()
            .execute_unprepared("ALTER TABLE strategy_task ALTER COLUMN a2a_task_id SET NOT NULL")
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(StrategyTask::Table)
                    .rename_column(StrategyTask::A2aTaskId, StrategyTask::KubeopencodeTaskName)
                    .to_owned(),
            )
            .await
    }
}
