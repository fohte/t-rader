use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20260926_171427_drop_jquants_plan_setting"
    }
}

#[derive(DeriveIden)]
enum JQuantsPlanSetting {
    #[sea_orm(iden = "jquants_plan_setting")]
    Table,
    Id,
    PlanSetting,
    UpdatedAt,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(JQuantsPlanSetting::Table).to_owned())
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
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
}
