use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20261115_122945_document_search"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(DocumentChunk::Table)
                    .add_column(
                        ColumnDef::new(DocumentChunk::SearchText)
                            .text()
                            .not_null()
                            .default(Expr::cust("''")),
                    )
                    .add_column(
                        ColumnDef::new(DocumentChunk::SearchTerms)
                            .array(ColumnType::Text)
                            .not_null()
                            .default(Expr::cust("ARRAY[]::text[]")),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_document_chunk_search_terms")
                    .table(DocumentChunk::Table)
                    .col(DocumentChunk::SearchTerms)
                    .index_type(IndexType::FullText)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .name("idx_document_chunk_search_terms")
                    .table(DocumentChunk::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(DocumentChunk::Table)
                    .drop_column(DocumentChunk::SearchTerms)
                    .drop_column(DocumentChunk::SearchText)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum DocumentChunk {
    Table,
    SearchText,
    SearchTerms,
}
