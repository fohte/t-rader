use sea_orm_migration::prelude::*;

pub struct Migration;

#[derive(DeriveIden)]
enum Trigger {
    Table,
    Purpose,
}

#[derive(DeriveIden)]
enum AgentConfig {
    Table,
    Purpose,
}

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20260929_183054_add_trigger_purpose"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Trigger::Table)
                    .add_column(ColumnDef::new(Trigger::Purpose).text())
                    .to_owned(),
            )
            .await?;
        let purpose_foreign_key = TableForeignKey::new()
            .name("fk_trigger_purpose_agent_config")
            .from_tbl(Trigger::Table)
            .from_col(Trigger::Purpose)
            .to_tbl(AgentConfig::Table)
            .to_col(AgentConfig::Purpose)
            .on_delete(ForeignKeyAction::SetNull)
            .to_owned();
        manager
            .alter_table(
                Table::alter()
                    .table(Trigger::Table)
                    .add_foreign_key(&purpose_foreign_key)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Trigger::Table)
                    .drop_column(Trigger::Purpose)
                    .to_owned(),
            )
            .await
    }
}
