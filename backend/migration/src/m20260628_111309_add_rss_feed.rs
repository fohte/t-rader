use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(DeriveIden)]
enum RssFeed {
    Table,
    Id,
    Source,
    DisplayName,
    Url,
    Enabled,
    CreatedAt,
    UpdatedAt,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(RssFeed::Table)
                    .col(
                        ColumnDef::new(RssFeed::Id)
                            .uuid()
                            .not_null()
                            .primary_key()
                            .default(Expr::cust("gen_random_uuid()")),
                    )
                    .col(
                        ColumnDef::new(RssFeed::Source)
                            .text()
                            .not_null()
                            .unique_key(),
                    )
                    .col(ColumnDef::new(RssFeed::DisplayName).text().not_null())
                    .col(ColumnDef::new(RssFeed::Url).text().not_null())
                    .col(
                        ColumnDef::new(RssFeed::Enabled)
                            .boolean()
                            .not_null()
                            .default(true),
                    )
                    .col(
                        ColumnDef::new(RssFeed::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(RssFeed::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(RssFeed::Table).to_owned())
            .await
    }
}
