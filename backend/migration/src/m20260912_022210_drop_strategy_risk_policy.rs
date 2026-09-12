use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20260912_022210_drop_strategy_risk_policy"
    }
}

#[derive(DeriveIden)]
enum Strategy {
    Table,
    RiskPolicy,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Strategy::Table)
                    .drop_column(Strategy::RiskPolicy)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Strategy::Table)
                    .add_column_if_not_exists(
                        ColumnDef::new(Strategy::RiskPolicy)
                            .json_binary()
                            .not_null()
                            .default(Expr::cust("'{}'::jsonb")),
                    )
                    .to_owned(),
            )
            .await
    }
}
