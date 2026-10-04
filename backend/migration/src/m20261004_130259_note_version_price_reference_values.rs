use sea_orm_migration::prelude::*;

pub struct Migration;

#[derive(DeriveIden)]
enum NoteVersion {
    Table,
    ResolvedPriceReferencesJson,
}

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20261004_130259_note_version_price_reference_values"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(NoteVersion::Table)
                    .add_column_if_not_exists(
                        ColumnDef::new(NoteVersion::ResolvedPriceReferencesJson)
                            .json_binary()
                            .not_null()
                            .default(Expr::cust("'{}'::jsonb")),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(NoteVersion::Table)
                    .drop_column(NoteVersion::ResolvedPriceReferencesJson)
                    .to_owned(),
            )
            .await
    }
}
