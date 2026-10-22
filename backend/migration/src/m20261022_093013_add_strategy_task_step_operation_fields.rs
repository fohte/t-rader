use sea_orm_migration::prelude::*;

pub struct Migration;

#[derive(DeriveIden)]
enum StrategyTaskStep {
    Table,
    Kind,
    Model,
    ItemKey,
    InputRef,
    OutputRef,
}

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20261022_093013_add_strategy_task_step_operation_fields"
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
                        ColumnDef::new(StrategyTaskStep::Kind)
                            .text()
                            .not_null()
                            .default("llm"),
                    )
                    .modify_column(ColumnDef::new(StrategyTaskStep::Model).text().null())
                    .add_column(ColumnDef::new(StrategyTaskStep::ItemKey).json_binary())
                    .add_column(ColumnDef::new(StrategyTaskStep::InputRef).json_binary())
                    .add_column(ColumnDef::new(StrategyTaskStep::OutputRef).json_binary())
                    .to_owned(),
            )
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(StrategyTaskStep::Table)
                    .drop_column(StrategyTaskStep::Kind)
                    .modify_column(ColumnDef::new(StrategyTaskStep::Model).text().not_null())
                    .drop_column(StrategyTaskStep::ItemKey)
                    .drop_column(StrategyTaskStep::InputRef)
                    .drop_column(StrategyTaskStep::OutputRef)
                    .to_owned(),
            )
            .await?;
        Ok(())
    }
}
