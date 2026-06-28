use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(DeriveIden)]
enum Strategy {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum NewsItem {
    Table,
    Id,
    Source,
    Url,
    Title,
    BodySnippet,
    PublishedAt,
    FetchedAt,
}

#[derive(DeriveIden)]
enum NewsStrategyLink {
    Table,
    NewsId,
    StrategyId,
    RefKind,
    RefId,
    MatchedTerm,
    CreatedAt,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(NewsItem::Table)
                    .col(
                        ColumnDef::new(NewsItem::Id)
                            .uuid()
                            .not_null()
                            .primary_key()
                            .default(Expr::cust("gen_random_uuid()")),
                    )
                    .col(ColumnDef::new(NewsItem::Source).text().not_null())
                    .col(ColumnDef::new(NewsItem::Url).text().not_null().unique_key())
                    .col(ColumnDef::new(NewsItem::Title).text().not_null())
                    .col(ColumnDef::new(NewsItem::BodySnippet).text())
                    .col(
                        ColumnDef::new(NewsItem::PublishedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(NewsItem::FetchedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .get_connection()
            .execute_unprepared(
                "CREATE INDEX news_item_published_at_idx \
                 ON news_item (published_at DESC)",
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(NewsStrategyLink::Table)
                    .col(ColumnDef::new(NewsStrategyLink::NewsId).uuid().not_null())
                    .col(
                        ColumnDef::new(NewsStrategyLink::StrategyId)
                            .uuid()
                            .not_null(),
                    )
                    .col(ColumnDef::new(NewsStrategyLink::RefKind).text().not_null())
                    .col(ColumnDef::new(NewsStrategyLink::RefId).text().not_null())
                    .col(
                        ColumnDef::new(NewsStrategyLink::MatchedTerm)
                            .text()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(NewsStrategyLink::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .primary_key(
                        Index::create()
                            .col(NewsStrategyLink::NewsId)
                            .col(NewsStrategyLink::StrategyId)
                            .col(NewsStrategyLink::RefKind)
                            .col(NewsStrategyLink::RefId),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(NewsStrategyLink::Table, NewsStrategyLink::NewsId)
                            .to(NewsItem::Table, NewsItem::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(NewsStrategyLink::Table, NewsStrategyLink::StrategyId)
                            .to(Strategy::Table, Strategy::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .get_connection()
            .execute_unprepared(
                "ALTER TABLE news_strategy_link \
                 ADD CONSTRAINT news_strategy_link_ref_kind_check \
                 CHECK (ref_kind IN ('stock', 'indicator', 'sector', 'theme'))",
            )
            .await?;

        manager
            .get_connection()
            .execute_unprepared(
                "CREATE INDEX news_strategy_link_strategy_idx \
                 ON news_strategy_link (strategy_id, news_id)",
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(NewsStrategyLink::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(NewsItem::Table).to_owned())
            .await?;
        Ok(())
    }
}
