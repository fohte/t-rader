use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(DeriveIden)]
enum StrategyTask {
    Table,
    AsOf,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(StrategyTask::Table)
                    .add_column_if_not_exists(
                        ColumnDef::new(StrategyTask::AsOf)
                            .timestamp_with_time_zone()
                            .null(),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(StrategyTask::Table)
                    .drop_column(StrategyTask::AsOf)
                    .to_owned(),
            )
            .await
    }
}
