use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(DeriveIden)]
enum SubagentDefinition {
    Table,
    Id,
    Key,
    Description,
    Model,
    ReasoningEffort,
    Prompt,
    Tools,
    TimeoutMinutes,
    MaxModelCalls,
    ToolLimits,
    Output,
    CreatedAt,
    UpdatedAt,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(SubagentDefinition::Table)
                    .col(
                        ColumnDef::new(SubagentDefinition::Id)
                            .uuid()
                            .not_null()
                            .primary_key()
                            .default(Expr::cust("gen_random_uuid()")),
                    )
                    .col(
                        ColumnDef::new(SubagentDefinition::Key)
                            .text()
                            .not_null()
                            .unique_key(),
                    )
                    .col(ColumnDef::new(SubagentDefinition::Description).text().not_null())
                    .col(ColumnDef::new(SubagentDefinition::Model).text().not_null())
                    .col(ColumnDef::new(SubagentDefinition::ReasoningEffort).text())
                    .col(ColumnDef::new(SubagentDefinition::Prompt).text().not_null())
                    .col(
                        ColumnDef::new(SubagentDefinition::Tools)
                            .json_binary()
                            .not_null()
                            .default(Expr::cust("'[]'::jsonb")),
                    )
                    .col(
                        ColumnDef::new(SubagentDefinition::TimeoutMinutes)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SubagentDefinition::MaxModelCalls)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SubagentDefinition::ToolLimits)
                            .json_binary()
                            .not_null()
                            .default(Expr::cust("'{}'::jsonb")),
                    )
                    .col(
                        ColumnDef::new(SubagentDefinition::Output)
                            .json_binary()
                            .not_null()
                            .default(Expr::cust("'{}'::jsonb")),
                    )
                    .col(
                        ColumnDef::new(SubagentDefinition::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(SubagentDefinition::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .check(
                        Expr::col(SubagentDefinition::TimeoutMinutes)
                            .gte(1)
                            .and(Expr::col(SubagentDefinition::TimeoutMinutes).lte(360)),
                    )
                    .check(Expr::col(SubagentDefinition::MaxModelCalls).gt(0))
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(SubagentDefinition::Table).to_owned())
            .await
    }
}
