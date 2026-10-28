use sea_orm_migration::prelude::*;

pub struct Migration;

#[derive(DeriveIden)]
enum StrategyTaskStep {
    Table,
    ModelCallLimitReached,
}

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20261028_174020_add_strategy_task_step_model_call_limit_reached"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(StrategyTaskStep::Table)
                    .add_column(
                        ColumnDef::new(StrategyTaskStep::ModelCallLimitReached)
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
                    .table(StrategyTaskStep::Table)
                    .drop_column(StrategyTaskStep::ModelCallLimitReached)
                    .to_owned(),
            )
            .await
    }
}
