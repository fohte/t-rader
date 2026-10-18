use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(DeriveIden)]
enum UsStockSipBarRefetchProgress {
    Table,
    InstrumentId,
    CompletedAt,
    SkippedReason,
}

#[derive(DeriveIden)]
enum UsStockSipBarRefetchWriteBudget {
    Table,
    Id,
    HourStartedAt,
    RowsWritten,
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
                    .table(UsStockSipBarRefetchProgress::Table)
                    .col(
                        ColumnDef::new(UsStockSipBarRefetchProgress::InstrumentId)
                            .text()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(UsStockSipBarRefetchProgress::CompletedAt)
                            .timestamp_with_time_zone(),
                    )
                    .col(ColumnDef::new(UsStockSipBarRefetchProgress::SkippedReason).text())
                    .foreign_key(
                        ForeignKey::create()
                            .from(
                                UsStockSipBarRefetchProgress::Table,
                                UsStockSipBarRefetchProgress::InstrumentId,
                            )
                            .to(Instruments::Table, Instruments::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .check(
                        Expr::col(UsStockSipBarRefetchProgress::CompletedAt)
                            .is_not_null()
                            .or(Expr::col(UsStockSipBarRefetchProgress::SkippedReason).is_not_null()),
                    )
                    .check(
                        Expr::col(UsStockSipBarRefetchProgress::CompletedAt)
                            .is_null()
                            .or(Expr::col(UsStockSipBarRefetchProgress::SkippedReason).is_null()),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .create_table(
                Table::create()
                    .table(UsStockSipBarRefetchWriteBudget::Table)
                    .col(
                        ColumnDef::new(UsStockSipBarRefetchWriteBudget::Id)
                            .small_integer()
                            .not_null()
                            .primary_key()
                            .default(1)
                            .check(Expr::col(UsStockSipBarRefetchWriteBudget::Id).eq(1)),
                    )
                    .col(
                        ColumnDef::new(UsStockSipBarRefetchWriteBudget::HourStartedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(UsStockSipBarRefetchWriteBudget::RowsWritten)
                            .big_integer()
                            .not_null()
                            .default(0)
                            .check(Expr::col(UsStockSipBarRefetchWriteBudget::RowsWritten).gte(0)),
                    )
                    .to_owned(),
            )
            .await?;

        let db = manager.get_connection();
        db.execute_unprepared(
            "COMMENT ON TABLE us_stock_sip_bar_refetch_progress IS '米国株の SIP 足を再取得した銘柄と、再取得できない理由を保持する。'",
        )
        .await?;
        db.execute_unprepared(
            "COMMENT ON COLUMN us_stock_sip_bar_refetch_progress.instrument_id IS 'SIP 足を再取得する銘柄。'",
        )
        .await?;
        db.execute_unprepared(
            "COMMENT ON COLUMN us_stock_sip_bar_refetch_progress.completed_at IS '指定期間の足を SIP で再取得した時刻。'",
        )
        .await?;
        db.execute_unprepared(
            "COMMENT ON COLUMN us_stock_sip_bar_refetch_progress.skipped_reason IS 'SIP で再取得できず、対象から除外した理由。'",
        )
        .await?;
        db.execute_unprepared(
            "COMMENT ON TABLE us_stock_sip_bar_refetch_write_budget IS '米国株 SIP 足の再取得で時間ごとに書き込んだ行数を保持する。'",
        )
        .await?;
        db.execute_unprepared(
            "COMMENT ON COLUMN us_stock_sip_bar_refetch_write_budget.hour_started_at IS '行数を集計する UTC 時間の開始時刻。'",
        )
        .await?;
        db.execute_unprepared(
            "COMMENT ON COLUMN us_stock_sip_bar_refetch_write_budget.rows_written IS 'この時間に SIP 再取得で upsert した行数。'",
        )
        .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(UsStockSipBarRefetchWriteBudget::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(UsStockSipBarRefetchProgress::Table).to_owned())
            .await
    }
}
