use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(DeriveIden)]
enum DocumentVersionRetrieval {
    Table,
    Id,
    VersionId,
    RetrievedAt,
}

#[derive(DeriveIden)]
enum DocumentVersion {
    Table,
    Id,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(DocumentVersionRetrieval::Table)
                    .col(
                        ColumnDef::new(DocumentVersionRetrieval::Id)
                            .uuid()
                            .not_null()
                            .default(Expr::cust("gen_random_uuid()"))
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(DocumentVersionRetrieval::VersionId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(DocumentVersionRetrieval::RetrievedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_document_version_retrieval_version")
                            .from(
                                DocumentVersionRetrieval::Table,
                                DocumentVersionRetrieval::VersionId,
                            )
                            .to(DocumentVersion::Table, DocumentVersion::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_document_version_retrieval_version_retrieved_at")
                    .table(DocumentVersionRetrieval::Table)
                    .col(DocumentVersionRetrieval::VersionId)
                    .col((DocumentVersionRetrieval::RetrievedAt, IndexOrder::Desc))
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(DocumentVersionRetrieval::Table).to_owned())
            .await
    }
}
