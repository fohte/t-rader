use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20261027_173631_add_us_stock_cik"
    }
}

#[derive(DeriveIden)]
enum Stock {
    Table,
    Cik,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Stock::Table)
                    .add_column(ColumnDef::new(Stock::Cik).string().null())
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Stock::Table)
                    .drop_column(Stock::Cik)
                    .to_owned(),
            )
            .await
    }
}
