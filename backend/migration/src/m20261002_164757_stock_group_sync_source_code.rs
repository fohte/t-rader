use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(StockGroup::Table)
                    .add_column(ColumnDef::new(StockGroup::SyncSourceCode).text())
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(StockGroup::Table)
                    .drop_column(StockGroup::SyncSourceCode)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum StockGroup {
    Table,
    SyncSourceCode,
}
