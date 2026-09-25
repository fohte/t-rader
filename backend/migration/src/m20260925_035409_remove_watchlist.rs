use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(DeriveIden)]
enum Watchlists {
    Table,
    Id,
    Name,
    SortOrder,
    CreatedAt,
}

#[derive(DeriveIden)]
enum WatchlistItems {
    Table,
    WatchlistId,
    InstrumentId,
    SortOrder,
    AddedAt,
}

#[derive(DeriveIden)]
enum Instruments {
    Table,
    Id,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(WatchlistItems::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Watchlists::Table).to_owned())
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Watchlists::Table)
                    .col(
                        ColumnDef::new(Watchlists::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Watchlists::Name).string().not_null())
                    .col(ColumnDef::new(Watchlists::SortOrder).integer().not_null())
                    .col(
                        ColumnDef::new(Watchlists::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(WatchlistItems::Table)
                    .col(
                        ColumnDef::new(WatchlistItems::WatchlistId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(WatchlistItems::InstrumentId)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(WatchlistItems::SortOrder)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(WatchlistItems::AddedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .primary_key(
                        Index::create()
                            .col(WatchlistItems::WatchlistId)
                            .col(WatchlistItems::InstrumentId),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(WatchlistItems::Table, WatchlistItems::WatchlistId)
                            .to(Watchlists::Table, Watchlists::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(WatchlistItems::Table, WatchlistItems::InstrumentId)
                            .to(Instruments::Table, Instruments::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}
