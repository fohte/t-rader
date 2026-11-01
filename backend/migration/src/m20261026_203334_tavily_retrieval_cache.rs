use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(DeriveIden)]
enum TavilyRetrievalCache {
    Table,
    CacheKey,
    Kind,
    PayloadJson,
    FetchedAt,
    ExpiresAt,
    LeaseToken,
    LeaseExpiresAt,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(TavilyRetrievalCache::Table)
                    .col(
                        ColumnDef::new(TavilyRetrievalCache::CacheKey)
                            .binary_len(32)
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(TavilyRetrievalCache::Kind)
                            .string_len(16)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(TavilyRetrievalCache::PayloadJson)
                            .json_binary()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(TavilyRetrievalCache::FetchedAt)
                            .timestamp_with_time_zone()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(TavilyRetrievalCache::ExpiresAt)
                            .timestamp_with_time_zone()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(TavilyRetrievalCache::LeaseToken)
                            .uuid()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(TavilyRetrievalCache::LeaseExpiresAt)
                            .timestamp_with_time_zone()
                            .null(),
                    )
                    .check(Expr::col(TavilyRetrievalCache::Kind).is_in(["search", "extract"]))
                    .check(
                        Expr::cust(
                            "(payload_json IS NOT NULL AND fetched_at IS NOT NULL AND expires_at IS NOT NULL AND lease_token IS NULL AND lease_expires_at IS NULL AND expires_at > fetched_at) OR (payload_json IS NULL AND fetched_at IS NULL AND expires_at IS NULL AND lease_token IS NOT NULL AND lease_expires_at IS NOT NULL)",
                        ),
                    )
                    .check(Expr::cust("octet_length(cache_key) = 32"))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_tavily_retrieval_cache_expires_at")
                    .table(TavilyRetrievalCache::Table)
                    .col(TavilyRetrievalCache::ExpiresAt)
                    .to_owned(),
            )
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(TavilyRetrievalCache::Table).to_owned())
            .await
    }
}
