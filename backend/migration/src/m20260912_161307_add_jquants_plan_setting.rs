use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(DeriveIden)]
enum JQuantsPlanSetting {
    // DeriveIden の既定の snake_case 変換は連続大文字 "JQ" を "j_q" と別語扱いしてしまう
    // (`j_quants_plan_setting` になる) ため、テーブル名は明示的に上書きする。
    #[sea_orm(iden = "jquants_plan_setting")]
    Table,
    Id,
    PlanSetting,
    UpdatedAt,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // 口座全体の設定を保持する単一行テーブル (id は常に 1 固定)。
        // 行が存在しない間は「未設定 (自動検出を使う)」を表す。
        manager
            .create_table(
                Table::create()
                    .table(JQuantsPlanSetting::Table)
                    .col(
                        ColumnDef::new(JQuantsPlanSetting::Id)
                            .small_integer()
                            .not_null()
                            .primary_key()
                            .default(1)
                            .check(Expr::col(JQuantsPlanSetting::Id).eq(1)),
                    )
                    .col(
                        ColumnDef::new(JQuantsPlanSetting::PlanSetting)
                            .json_binary()
                            .not_null()
                            .default(Expr::cust("'{}'::jsonb")),
                    )
                    .col(
                        ColumnDef::new(JQuantsPlanSetting::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(JQuantsPlanSetting::Table).to_owned())
            .await
    }
}
