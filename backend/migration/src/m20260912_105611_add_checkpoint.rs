use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(DeriveIden)]
enum Strategy {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum Checkpoint {
    Table,
    Id,
    StrategyId,
    Graph,
    Stream,
    Cursor,
    UpdatedByRunId,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum Idx {
    #[sea_orm(iden = "checkpoint_strategy_graph_stream_idx")]
    CheckpointStrategyGraphStream,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Checkpoint::Table)
                    .col(
                        ColumnDef::new(Checkpoint::Id)
                            .uuid()
                            .not_null()
                            .primary_key()
                            .default(Expr::cust("gen_random_uuid()")),
                    )
                    .col(ColumnDef::new(Checkpoint::StrategyId).uuid().not_null())
                    .col(ColumnDef::new(Checkpoint::Graph).text().not_null())
                    .col(ColumnDef::new(Checkpoint::Stream).text().not_null())
                    .col(ColumnDef::new(Checkpoint::Cursor).text().not_null())
                    .col(ColumnDef::new(Checkpoint::UpdatedByRunId).text())
                    .col(
                        ColumnDef::new(Checkpoint::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(Checkpoint::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(Checkpoint::Table, Checkpoint::StrategyId)
                            .to(Strategy::Table, Strategy::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name(Idx::CheckpointStrategyGraphStream.to_string())
                    .table(Checkpoint::Table)
                    .col(Checkpoint::StrategyId)
                    .col(Checkpoint::Graph)
                    .col(Checkpoint::Stream)
                    .unique()
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Checkpoint::Table).to_owned())
            .await
    }
}
