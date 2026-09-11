use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(DeriveIden)]
enum StrategyInterest {
    Table,
    Status,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(StrategyInterest::Table)
                    .add_column(
                        ColumnDef::new(StrategyInterest::Status)
                            .string()
                            .not_null()
                            .default("active"),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .get_connection()
            .execute_unprepared(
                "ALTER TABLE strategy_interest \
                 ADD CONSTRAINT strategy_interest_status_check \
                 CHECK (status IN ('active', 'archived'))",
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
                "ALTER TABLE strategy_interest DROP CONSTRAINT strategy_interest_status_check",
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(StrategyInterest::Table)
                    .drop_column(StrategyInterest::Status)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}
