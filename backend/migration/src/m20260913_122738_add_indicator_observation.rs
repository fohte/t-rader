use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20260913_122738_add_indicator_observation"
    }
}

#[derive(DeriveIden)]
enum Indicator {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum IndicatorObservation {
    Table,
    IndicatorId,
    Date,
    Value,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(IndicatorObservation::Table)
                    .col(
                        ColumnDef::new(IndicatorObservation::IndicatorId)
                            .string()
                            .not_null(),
                    )
                    .col(ColumnDef::new(IndicatorObservation::Date).date().not_null())
                    .col(
                        ColumnDef::new(IndicatorObservation::Value)
                            .decimal()
                            .not_null(),
                    )
                    .primary_key(
                        Index::create()
                            .col(IndicatorObservation::IndicatorId)
                            .col(IndicatorObservation::Date),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(
                                IndicatorObservation::Table,
                                IndicatorObservation::IndicatorId,
                            )
                            .to(Indicator::Table, Indicator::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(IndicatorObservation::Table).to_owned())
            .await
    }
}
