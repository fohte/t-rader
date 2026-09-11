use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20260911_005614_drop_strategy_agent_columns"
    }
}

#[derive(DeriveIden)]
enum Strategy {
    Table,
    AgentsMd,
    Skills,
    AgentGraph,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Strategy::Table)
                    .drop_column(Strategy::AgentsMd)
                    .drop_column(Strategy::Skills)
                    .drop_column(Strategy::AgentGraph)
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
                        ColumnDef::new(Strategy::AgentGraph)
                            .text()
                            .not_null()
                            .default(""),
                    )
                    .to_owned(),
            )
            .await
    }
}
