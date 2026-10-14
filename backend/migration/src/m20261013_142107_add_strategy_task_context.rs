use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(DeriveIden)]
enum StrategyTask {
    Table,
    ContextKind,
    ContextId,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(StrategyTask::Table)
                    .add_column(
                        ColumnDef::new(StrategyTask::ContextKind)
                            .text()
                            .null(),
                    )
                    .add_column(ColumnDef::new(StrategyTask::ContextId).text().null())
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(StrategyTask::Table)
                    .drop_column(StrategyTask::ContextId)
                    .drop_column(StrategyTask::ContextKind)
                    .to_owned(),
            )
            .await
    }
}
