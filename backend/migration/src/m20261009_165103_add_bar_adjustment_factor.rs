use sea_orm_migration::prelude::*;

pub struct Migration;

#[derive(DeriveIden)]
enum Bars {
    Table,
    AdjustmentFactor,
}

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20261009_165103_add_bar_adjustment_factor"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Bars::Table)
                    .add_column(
                        ColumnDef::new(Bars::AdjustmentFactor)
                            .decimal()
                            .not_null()
                            .default(Expr::cust("1")),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Bars::Table)
                    .drop_column(Bars::AdjustmentFactor)
                    .to_owned(),
            )
            .await
    }
}
