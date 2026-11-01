use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
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
        manager
            .get_connection()
            .execute_unprepared(
                "ALTER TABLE strategy_task_step \
                 ADD CONSTRAINT strategy_task_step_kind_check \
                 CHECK (kind IN ('llm', 'operation'))",
            )
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
                "UPDATE strategy_task_step SET model = '' WHERE model IS NULL; \
                 ALTER TABLE strategy_task_step \
                 DROP CONSTRAINT strategy_task_step_kind_check",
            )
            .await?;
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
