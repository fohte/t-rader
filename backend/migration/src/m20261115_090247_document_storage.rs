use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(DeriveIden)]
enum Document {
    Table,
    Id,
    Source,
    SourceDocumentId,
    StockId,
    SourceIssuerCode,
    Title,
    PublishedAt,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum DocumentVersion {
    Table,
    Id,
    DocumentId,
    ContentHash,
    RetrievedAt,
    Body,
    CreatedAt,
}

#[derive(DeriveIden)]
enum DocumentChunk {
    Table,
    Id,
    VersionId,
    Sequence,
    StartOffset,
    EndOffset,
    Body,
}

#[derive(DeriveIden)]
enum Stock {
    Table,
    Id,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Document::Table)
                    .col(
                        ColumnDef::new(Document::Id)
                            .uuid()
                            .not_null()
                            .default(Expr::cust("gen_random_uuid()"))
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Document::Source).text().not_null())
                    .col(ColumnDef::new(Document::SourceDocumentId).text().not_null())
                    .col(
                        ColumnDef::new(Document::StockId)
                            .text()
                            .null(),
                    )
                    .col(ColumnDef::new(Document::SourceIssuerCode).text().not_null())
                    .col(ColumnDef::new(Document::Title).text().not_null())
                    .col(
                        ColumnDef::new(Document::PublishedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Document::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(Document::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_document_stock")
                            .from(Document::Table, Document::StockId)
                            .to(Stock::Table, Stock::Id)
                            .on_delete(ForeignKeyAction::SetNull),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_document_source_document_id")
                    .table(Document::Table)
                    .col(Document::Source)
                    .col(Document::SourceDocumentId)
                    .unique()
                    .to_owned(),
            )
            .await?;
        manager
            .create_table(
                Table::create()
                    .table(DocumentVersion::Table)
                    .col(
                        ColumnDef::new(DocumentVersion::Id)
                            .uuid()
                            .not_null()
                            .default(Expr::cust("gen_random_uuid()"))
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(DocumentVersion::DocumentId)
                            .uuid()
                            .not_null(),
                    )
                    .col(ColumnDef::new(DocumentVersion::ContentHash).string_len(64).not_null())
                    .col(
                        ColumnDef::new(DocumentVersion::RetrievedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(ColumnDef::new(DocumentVersion::Body).text().not_null())
                    .col(
                        ColumnDef::new(DocumentVersion::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_document_version_document")
                            .from(DocumentVersion::Table, DocumentVersion::DocumentId)
                            .to(Document::Table, Document::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_document_version_document_retrieved_at")
                    .table(DocumentVersion::Table)
                    .col(DocumentVersion::DocumentId)
                    .col((DocumentVersion::RetrievedAt, IndexOrder::Desc))
                    .to_owned(),
            )
            .await?;
        manager
            .create_table(
                Table::create()
                    .table(DocumentChunk::Table)
                    .col(
                        ColumnDef::new(DocumentChunk::Id)
                            .uuid()
                            .not_null()
                            .default(Expr::cust("gen_random_uuid()"))
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(DocumentChunk::VersionId)
                            .uuid()
                            .not_null(),
                    )
                    .col(ColumnDef::new(DocumentChunk::Sequence).integer().not_null())
                    .col(ColumnDef::new(DocumentChunk::StartOffset).integer().not_null())
                    .col(ColumnDef::new(DocumentChunk::EndOffset).integer().not_null())
                    .col(ColumnDef::new(DocumentChunk::Body).text().not_null())
                    .check(Expr::col(DocumentChunk::Sequence).gte(0))
                    .check(Expr::col(DocumentChunk::StartOffset).gte(0))
                    .check(Expr::col(DocumentChunk::EndOffset).gte(Expr::col(DocumentChunk::StartOffset)))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_document_chunk_version")
                            .from(DocumentChunk::Table, DocumentChunk::VersionId)
                            .to(DocumentVersion::Table, DocumentVersion::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_document_chunk_version_sequence")
                    .table(DocumentChunk::Table)
                    .col(DocumentChunk::VersionId)
                    .col(DocumentChunk::Sequence)
                    .unique()
                    .to_owned(),
            )
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(DocumentChunk::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(DocumentVersion::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Document::Table).to_owned())
            .await
    }
}
