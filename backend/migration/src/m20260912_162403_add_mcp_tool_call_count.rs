use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(DeriveIden)]
enum McpToolCallCount {
    Table,
    Id,
    TaskExecutionId,
    ToolName,
    CallCount,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum Idx {
    #[sea_orm(iden = "mcp_tool_call_count_task_tool_idx")]
    McpToolCallCountTaskTool,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(McpToolCallCount::Table)
                    .col(
                        ColumnDef::new(McpToolCallCount::Id)
                            .uuid()
                            .not_null()
                            .primary_key()
                            .default(Expr::cust("gen_random_uuid()")),
                    )
                    .col(
                        ColumnDef::new(McpToolCallCount::TaskExecutionId)
                            .text()
                            .not_null(),
                    )
                    .col(ColumnDef::new(McpToolCallCount::ToolName).text().not_null())
                    .col(
                        ColumnDef::new(McpToolCallCount::CallCount)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(McpToolCallCount::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(McpToolCallCount::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name(Idx::McpToolCallCountTaskTool.to_string())
                    .table(McpToolCallCount::Table)
                    .col(McpToolCallCount::TaskExecutionId)
                    .col(McpToolCallCount::ToolName)
                    .unique()
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(McpToolCallCount::Table).to_owned())
            .await
    }
}
