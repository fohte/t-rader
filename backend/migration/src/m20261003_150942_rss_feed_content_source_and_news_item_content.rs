use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(DeriveIden)]
enum RssFeed {
    Table,
    ContentSource,
}

#[derive(DeriveIden)]
enum NewsItem {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum NewsItemContent {
    Table,
    NewsItemId,
    Status,
    Body,
    Error,
    CreatedAt,
    UpdatedAt,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(RssFeed::Table)
                    .add_column(
                        ColumnDef::new(RssFeed::ContentSource)
                            .text()
                            .not_null()
                            .default("none"),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .get_connection()
            .execute_unprepared(
                "ALTER TABLE rss_feed \
                 ADD CONSTRAINT rss_feed_content_source_check \
                 CHECK (content_source IN ('none', 'feed', 'crawl'))",
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(NewsItemContent::Table)
                    .col(
                        ColumnDef::new(NewsItemContent::NewsItemId)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(NewsItemContent::Status).text().not_null())
                    .col(ColumnDef::new(NewsItemContent::Body).text())
                    .col(ColumnDef::new(NewsItemContent::Error).text())
                    .col(
                        ColumnDef::new(NewsItemContent::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(NewsItemContent::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(NewsItemContent::Table, NewsItemContent::NewsItemId)
                            .to(NewsItem::Table, NewsItem::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .get_connection()
            .execute_unprepared(
                "ALTER TABLE news_item_content \
                 ADD CONSTRAINT news_item_content_status_check \
                 CHECK (status IN ('pending', 'fetched', 'failed'))",
            )
            .await?;
        manager
            .get_connection()
            .execute_unprepared(
                "ALTER TABLE news_item_content \
                 ADD CONSTRAINT news_item_content_fetched_body_check \
                 CHECK ((status = 'fetched') = (body IS NOT NULL))",
            )
            .await?;
        manager
            .get_connection()
            .execute_unprepared(
                "ALTER TABLE news_item_content \
                 ADD CONSTRAINT news_item_content_failed_error_check \
                 CHECK ((status = 'failed') = (error IS NOT NULL))",
            )
            .await?;
        manager
            .get_connection()
            .execute_unprepared(
                "CREATE INDEX news_item_content_pending_created_at_idx \
                 ON news_item_content (created_at) WHERE status = 'pending'",
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(NewsItemContent::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(RssFeed::Table)
                    .drop_column(RssFeed::ContentSource)
                    .to_owned(),
            )
            .await?;
        Ok(())
    }
}
