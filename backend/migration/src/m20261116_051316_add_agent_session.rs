use sea_orm_migration::prelude::*;

pub struct Migration;

#[derive(DeriveIden)]
enum AgentConfig {
    Table,
    AgentSession,
}

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20261116_051316_add_agent_session"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(AgentConfig::Table)
                    .add_column(
                        ColumnDef::new(AgentConfig::AgentSession)
                            .text()
                            .not_null()
                            .default(""),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(AgentConfig::Table)
                    .drop_column(AgentConfig::AgentSession)
                    .to_owned(),
            )
            .await
    }
}
