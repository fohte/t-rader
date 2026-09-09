use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20260909_165900_make_hypothesis_trigger_strategy_id_nullable"
    }
}

#[derive(DeriveIden)]
enum Hypothesis {
    Table,
    StrategyId,
}

#[derive(DeriveIden)]
enum Trigger {
    Table,
    StrategyId,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Hypothesis::Table)
                    .modify_column(ColumnDef::new(Hypothesis::StrategyId).uuid().null())
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Trigger::Table)
                    .modify_column(ColumnDef::new(Trigger::StrategyId).uuid().null())
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Hypothesis::Table)
                    .modify_column(ColumnDef::new(Hypothesis::StrategyId).uuid().not_null())
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Trigger::Table)
                    .modify_column(ColumnDef::new(Trigger::StrategyId).uuid().not_null())
                    .to_owned(),
            )
            .await
    }
}
