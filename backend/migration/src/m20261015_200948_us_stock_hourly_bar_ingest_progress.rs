use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(DeriveIden)]
enum UsStockHourlyBarIngestProgress {
    Table,
    InstrumentId,
    CompletedThrough,
}

#[derive(DeriveIden)]
enum Instruments {
    Table,
    Id,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(UsStockHourlyBarIngestProgress::Table)
                    .col(
                        ColumnDef::new(UsStockHourlyBarIngestProgress::InstrumentId)
                            .text()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(UsStockHourlyBarIngestProgress::CompletedThrough)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(
                                UsStockHourlyBarIngestProgress::Table,
                                UsStockHourlyBarIngestProgress::InstrumentId,
                            )
                            .to(Instruments::Table, Instruments::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .get_connection()
            .execute_unprepared(
                "COMMENT ON TABLE us_stock_hourly_bar_ingest_progress IS '米国株の 1 時間足を取得済みの終端時刻を保持する。';",
            )
            .await?;
        manager
            .get_connection()
            .execute_unprepared(
                "COMMENT ON COLUMN us_stock_hourly_bar_ingest_progress.instrument_id IS '取得対象の銘柄。';",
            )
            .await?;
        manager
            .get_connection()
            .execute_unprepared(
                "COMMENT ON COLUMN us_stock_hourly_bar_ingest_progress.completed_through IS 'この時刻まで 1 時間足 API の取得が完了した。';",
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(UsStockHourlyBarIngestProgress::Table)
                    .to_owned(),
            )
            .await
    }
}
