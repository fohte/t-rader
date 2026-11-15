use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(DeriveIden)]
enum DocumentChunk {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum Stock {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum DocumentChunkMention {
    Table,
    Id,
    ChunkId,
    StockId,
    StartOffset,
    EndOffset,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(DocumentChunkMention::Table)
                    .col(
                        ColumnDef::new(DocumentChunkMention::Id)
                            .uuid()
                            .not_null()
                            .default(Expr::cust("gen_random_uuid()"))
                            .primary_key(),
                    )
                    .col(ColumnDef::new(DocumentChunkMention::ChunkId).uuid().not_null())
                    .col(ColumnDef::new(DocumentChunkMention::StockId).text().not_null())
                    .col(
                        ColumnDef::new(DocumentChunkMention::StartOffset)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(DocumentChunkMention::EndOffset)
                            .integer()
                            .not_null(),
                    )
                    .check(Expr::col(DocumentChunkMention::StartOffset).gte(0))
                    .check(
                        Expr::col(DocumentChunkMention::EndOffset)
                            .gt(Expr::col(DocumentChunkMention::StartOffset)),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_document_chunk_mention_chunk")
                            .from(DocumentChunkMention::Table, DocumentChunkMention::ChunkId)
                            .to(DocumentChunk::Table, DocumentChunk::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_document_chunk_mention_stock")
                            .from(DocumentChunkMention::Table, DocumentChunkMention::StockId)
                            .to(Stock::Table, Stock::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_document_chunk_mention_unique")
                    .table(DocumentChunkMention::Table)
                    .col(DocumentChunkMention::ChunkId)
                    .col(DocumentChunkMention::StockId)
                    .col(DocumentChunkMention::StartOffset)
                    .col(DocumentChunkMention::EndOffset)
                    .unique()
                    .to_owned(),
            )
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(DocumentChunkMention::Table).to_owned())
            .await
    }
}
