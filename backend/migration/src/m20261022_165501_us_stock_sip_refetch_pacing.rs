use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(DeriveIden)]
enum UsStockSipBarRefetchWriteBudget {
    Table,
    NextAllowedAt,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(UsStockSipBarRefetchWriteBudget::Table)
                    .add_column(
                        ColumnDef::new(UsStockSipBarRefetchWriteBudget::NextAllowedAt)
                            .timestamp_with_time_zone(),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .get_connection()
            .execute_unprepared(
                "COMMENT ON COLUMN us_stock_sip_bar_refetch_write_budget.next_allowed_at IS '次の SIP ページ書き込みが許可される時刻。'",
            )
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(UsStockSipBarRefetchWriteBudget::Table)
                    .drop_column(UsStockSipBarRefetchWriteBudget::NextAllowedAt)
                    .to_owned(),
            )
            .await
    }
}
