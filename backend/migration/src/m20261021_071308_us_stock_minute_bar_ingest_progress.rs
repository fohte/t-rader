use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(DeriveIden)]
enum UsStockMinuteBarIngestProgress {
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
                    .table(UsStockMinuteBarIngestProgress::Table)
                    .col(
                        ColumnDef::new(UsStockMinuteBarIngestProgress::InstrumentId)
                            .text()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(UsStockMinuteBarIngestProgress::CompletedThrough)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(
                                UsStockMinuteBarIngestProgress::Table,
                                UsStockMinuteBarIngestProgress::InstrumentId,
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
                "COMMENT ON TABLE us_stock_minute_bar_ingest_progress IS '米国株の 1 分足を取得済みの終端時刻を保持する。';",
            )
            .await?;
        manager
            .get_connection()
            .execute_unprepared(
                "COMMENT ON COLUMN us_stock_minute_bar_ingest_progress.instrument_id IS '取得対象の銘柄。';",
            )
            .await?;
        manager
            .get_connection()
            .execute_unprepared(
                "COMMENT ON COLUMN us_stock_minute_bar_ingest_progress.completed_through IS 'この時刻まで 1 分足 API の取得が完了した。';",
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(UsStockMinuteBarIngestProgress::Table)
                    .to_owned(),
            )
            .await
    }
}
