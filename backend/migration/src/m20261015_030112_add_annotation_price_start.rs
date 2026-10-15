use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(DeriveIden)]
enum Annotation {
    Table,
    PriceStart,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Annotation::Table)
                    .add_column(
                        ColumnDef::new(Annotation::PriceStart)
                            .decimal()
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
                    .table(Annotation::Table)
                    .drop_column(Annotation::PriceStart)
                    .to_owned(),
            )
            .await
    }
}
