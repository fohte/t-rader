use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20260913_081737_add_ref_term"
    }
}

const REF_KINDS: [&str; 4] = ["stock", "indicator", "sector", "theme"];
const ORIGINS: [&str; 2] = ["human", "llm"];

#[derive(DeriveIden)]
enum RefTerm {
    Table,
    RefKind,
    RefId,
    Term,
    Origin,
    CreatedAt,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(RefTerm::Table)
                    .col(ColumnDef::new(RefTerm::RefKind).string().not_null())
                    .col(ColumnDef::new(RefTerm::RefId).string().not_null())
                    .col(ColumnDef::new(RefTerm::Term).string().not_null())
                    .col(ColumnDef::new(RefTerm::Origin).string().not_null())
                    .col(
                        ColumnDef::new(RefTerm::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .primary_key(
                        Index::create()
                            .col(RefTerm::RefKind)
                            .col(RefTerm::RefId)
                            .col(RefTerm::Term),
                    )
                    .check(Expr::col(RefTerm::RefKind).is_in(REF_KINDS))
                    .check(Expr::col(RefTerm::Origin).is_in(ORIGINS))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_ref_term_ref_kind_id")
                    .table(RefTerm::Table)
                    .col(RefTerm::RefKind)
                    .col(RefTerm::RefId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(RefTerm::Table).to_owned())
            .await?;
        Ok(())
    }
}
