use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

/// instruments テーブルのカラム識別子
#[derive(DeriveIden)]
enum Instruments {
    Table,
    Id,
    Name,
    Market,
    Sector,
}

/// watchlists テーブルのカラム識別子
#[derive(DeriveIden)]
enum Watchlists {
    Table,
    Id,
    Name,
    SortOrder,
    CreatedAt,
}

/// watchlist_items テーブルのカラム識別子
#[derive(DeriveIden)]
enum WatchlistItems {
    Table,
    WatchlistId,
    InstrumentId,
    SortOrder,
    AddedAt,
}

/// bars テーブルのカラム識別子
#[derive(DeriveIden)]
enum Bars {
    Table,
    InstrumentId,
    Timeframe,
    Timestamp,
    Open,
    High,
    Low,
    Close,
    Volume,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        // TimescaleDB 拡張の有効化
        db.execute_unprepared("CREATE EXTENSION IF NOT EXISTS timescaledb")
            .await?;

        // instruments テーブル
        manager
            .create_table(
                Table::create()
                    .table(Instruments::Table)
                    .col(
                        ColumnDef::new(Instruments::Id)
                            .string()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Instruments::Name).string().not_null())
                    .col(ColumnDef::new(Instruments::Market).string().not_null())
                    .col(ColumnDef::new(Instruments::Sector).string())
                    .check(Expr::col(Instruments::Market).is_in(["TSE"]))
                    .to_owned(),
            )
            .await?;

        // watchlists テーブル
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

        // watchlist_items テーブル
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

        // bars テーブル
        manager
            .create_table(
                Table::create()
                    .table(Bars::Table)
                    .col(ColumnDef::new(Bars::InstrumentId).string().not_null())
                    .col(ColumnDef::new(Bars::Timeframe).string().not_null())
                    .col(
                        ColumnDef::new(Bars::Timestamp)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(ColumnDef::new(Bars::Open).decimal().not_null())
                    .col(ColumnDef::new(Bars::High).decimal().not_null())
                    .col(ColumnDef::new(Bars::Low).decimal().not_null())
                    .col(ColumnDef::new(Bars::Close).decimal().not_null())
                    .col(ColumnDef::new(Bars::Volume).big_integer().not_null())
                    .primary_key(
                        Index::create()
                            .col(Bars::InstrumentId)
                            .col(Bars::Timeframe)
                            .col(Bars::Timestamp),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(Bars::Table, Bars::InstrumentId)
                            .to(Instruments::Table, Instruments::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .check(Expr::col(Bars::Timeframe).is_in(["1d"]))
                    .to_owned(),
            )
            .await?;

        // TimescaleDB hypertable 化 (sea-query DSL では表現できないため raw SQL)
        db.execute_unprepared("SELECT create_hypertable('bars', by_range('timestamp'))")
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // 外部キー依存の逆順で削除
        manager
            .drop_table(Table::drop().table(Bars::Table).cascade().to_owned())
            .await?;
        manager
            .drop_table(
                Table::drop()
                    .table(WatchlistItems::Table)
                    .cascade()
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(Table::drop().table(Watchlists::Table).cascade().to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Instruments::Table).cascade().to_owned())
            .await?;

        Ok(())
    }
}
