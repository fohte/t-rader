use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20261007_163642_trigger_business_days_only"
    }
}

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
