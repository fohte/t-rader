use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20260913_122949_add_jquants_daily_bars_ingested_date"
    }
}

#[derive(DeriveIden)]
enum JQuantsDailyBarsIngestedDate {
    // DeriveIden の既定の snake_case 変換は連続大文字 "JQ" を "j_q" と別語扱いしてしまう
    // (`j_quants_daily_bars_ingested_date` になる) ため、テーブル名は明示的に上書きする。
    #[sea_orm(iden = "jquants_daily_bars_ingested_date")]
    Table,
    Date,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // 日足の全銘柄一括取り込みが完了した営業日を記録する。
        manager
            .create_table(
                Table::create()
                    .table(JQuantsDailyBarsIngestedDate::Table)
                    .col(
                        ColumnDef::new(JQuantsDailyBarsIngestedDate::Date)
                            .date()
                            .not_null()
                            .primary_key(),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(JQuantsDailyBarsIngestedDate::Table)
                    .to_owned(),
            )
            .await
    }
}
