use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20261010_084957_annotation_timestamp_start"
    }
}

#[derive(DeriveIden)]
enum Annotation {
    Table,
    TimestampStart,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Annotation::Table)
                    .add_column(
                        ColumnDef::new(Annotation::TimestampStart)
                            .timestamp_with_time_zone()
                            .null(),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Annotation::Table)
                    .drop_column(Annotation::TimestampStart)
                    .to_owned(),
            )
            .await
    }
}
