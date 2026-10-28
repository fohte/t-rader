use sea_orm_migration::prelude::*;

#[derive(DeriveIden)]
enum UsStockMinuteBarDownsampleCursor {
    Table,
    SingletonKey,
    CursorTimestamp,
}

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20261028_155642_us_stock_minute_bar_downsample_cursor"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(UsStockMinuteBarDownsampleCursor::Table)
                    .col(
                        ColumnDef::new(UsStockMinuteBarDownsampleCursor::SingletonKey)
                            .small_integer()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(UsStockMinuteBarDownsampleCursor::CursorTimestamp)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .check(Expr::col(UsStockMinuteBarDownsampleCursor::SingletonKey).eq(1))
                    .to_owned(),
            )
            .await?;
        manager
            .get_connection()
            .execute_unprepared(
                "COMMENT ON TABLE us_stock_minute_bar_downsample_cursor IS '米国株の 1 分足 downsample の再開位置を保持する。';",
            )
            .await?;
        manager
            .get_connection()
            .execute_unprepared(
                "COMMENT ON COLUMN us_stock_minute_bar_downsample_cursor.singleton_key IS '単一行を識別する固定キー。';",
            )
            .await?;
        manager
            .get_connection()
            .execute_unprepared(
                "COMMENT ON COLUMN us_stock_minute_bar_downsample_cursor.cursor_timestamp IS 'この時刻より前に米国株の 1 分足が存在しないことを示す。';",
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(UsStockMinuteBarDownsampleCursor::Table)
                    .to_owned(),
            )
            .await
    }
}
