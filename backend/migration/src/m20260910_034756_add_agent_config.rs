use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(DeriveIden)]
enum AgentConfig {
    Table,
    Id,
    Purpose,
    AgentsMd,
    Skills,
    AgentGraph,
    CreatedAt,
    UpdatedAt,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(AgentConfig::Table)
                    .col(
                        ColumnDef::new(AgentConfig::Id)
                            .uuid()
                            .not_null()
                            .primary_key()
                            .default(Expr::cust("gen_random_uuid()")),
                    )
                    .col(
                        ColumnDef::new(AgentConfig::Purpose)
                            .text()
                            .not_null()
                            .unique_key(),
                    )
                    .col(
                        ColumnDef::new(AgentConfig::AgentsMd)
                            .text()
                            .not_null()
                            .default(""),
                    )
                    .col(
                        ColumnDef::new(AgentConfig::Skills)
                            .json_binary()
                            .not_null()
                            .default(Expr::cust("'{}'::jsonb")),
                    )
                    .col(
                        ColumnDef::new(AgentConfig::AgentGraph)
                            .text()
                            .not_null()
                            .default(""),
                    )
                    .col(
                        ColumnDef::new(AgentConfig::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(AgentConfig::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .to_owned(),
            )
            .await?;

        // 既存の唯一の戦略が持つ agents_md/skills/agent_graph を、目的キー "default" の
        // 初期行として引き継ぐ (戦略が 0 件の環境では何も挿入されない)。
        manager
            .get_connection()
            .execute_unprepared(
                "INSERT INTO agent_config (purpose, agents_md, skills, agent_graph) \
                 SELECT 'default', agents_md, skills, agent_graph FROM strategy \
                 ORDER BY created_at ASC LIMIT 1",
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(AgentConfig::Table).to_owned())
            .await
    }
}
