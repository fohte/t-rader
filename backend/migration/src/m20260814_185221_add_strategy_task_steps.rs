use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(DeriveIden)]
enum StrategyTask {
    Table,
    Steps,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // steps は NOT NULL のため、既存行向けに一旦 DEFAULT 付きで追加してから
        // DEFAULT を外す。以降の INSERT では明示指定を必須にする。
        manager
            .get_connection()
            .execute_unprepared(
                "ALTER TABLE strategy_task ADD COLUMN steps jsonb NOT NULL DEFAULT '[]'::jsonb",
            )
            .await?;
        manager
            .get_connection()
            .execute_unprepared("ALTER TABLE strategy_task ALTER COLUMN steps DROP DEFAULT")
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(StrategyTask::Table)
                    .drop_column(StrategyTask::Steps)
                    .to_owned(),
            )
            .await
    }
}
