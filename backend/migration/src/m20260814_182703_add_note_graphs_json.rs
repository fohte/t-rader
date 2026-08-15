use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(DeriveIden)]
enum Note {
    Table,
    GraphsJson,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Note::Table)
                    .add_column_if_not_exists(
                        ColumnDef::new(Note::GraphsJson)
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
                    .table(Note::Table)
                    .drop_column(Note::GraphsJson)
                    .to_owned(),
            )
            .await
    }
}
