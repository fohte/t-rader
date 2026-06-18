use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(DeriveIden)]
enum Strategy {
    Table,
    AgentsMd,
    Skills,
    AgentStatus,
    AgentError,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Strategy::Table)
                    .add_column_if_not_exists(
                        ColumnDef::new(Strategy::AgentsMd)
                            .text()
                            .not_null()
                            .default(""),
                    )
                    .add_column_if_not_exists(
                        ColumnDef::new(Strategy::Skills)
                            .json_binary()
                            .not_null()
                            .default(Expr::cust("'{}'::jsonb")),
                    )
                    .add_column_if_not_exists(
                        ColumnDef::new(Strategy::AgentStatus)
                            .text()
                            .not_null()
                            .default("Pending"),
                    )
                    .add_column_if_not_exists(ColumnDef::new(Strategy::AgentError).text())
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Strategy::Table)
                    .drop_column(Strategy::AgentError)
                    .drop_column(Strategy::AgentStatus)
                    .drop_column(Strategy::Skills)
                    .drop_column(Strategy::AgentsMd)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}
