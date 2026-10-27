use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(DeriveIden)]
enum NoteVersion {
    Table,
    WarningDispositionsJson,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(NoteVersion::Table)
                    .add_column(
                        ColumnDef::new(NoteVersion::WarningDispositionsJson)
                            .json_binary()
                            .not_null()
                            .default(Expr::cust("'[]'::jsonb")),
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
                    .drop_column(NoteVersion::WarningDispositionsJson)
                    .to_owned(),
            )
            .await
    }
}
