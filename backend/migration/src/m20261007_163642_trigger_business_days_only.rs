use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(DeriveIden)]
enum Trigger {
    Table,
    BusinessDaysOnly,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Trigger::Table)
                    .add_column(
                        ColumnDef::new(Trigger::BusinessDaysOnly)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Trigger::Table)
                    .drop_column(Trigger::BusinessDaysOnly)
                    .to_owned(),
            )
            .await
    }
}
