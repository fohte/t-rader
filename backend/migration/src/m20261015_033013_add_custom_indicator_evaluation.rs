use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20261015_033013_add_custom_indicator_evaluation"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(CustomIndicatorEvaluation::Table)
                    .col(
                        ColumnDef::new(CustomIndicatorEvaluation::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    // 指標定義は削除できるため、評価記録からは外部キーを張らない。
                    .col(
                        ColumnDef::new(CustomIndicatorEvaluation::IndicatorId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(CustomIndicatorEvaluation::IndicatorName)
                            .text()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(CustomIndicatorEvaluation::Code)
                            .text()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(CustomIndicatorEvaluation::Args)
                            .json_binary()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(CustomIndicatorEvaluation::Output)
                            .json_binary()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(CustomIndicatorEvaluation::EvaluatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(CustomIndicatorEvaluation::Table)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum CustomIndicatorEvaluation {
    Table,
    Id,
    IndicatorId,
    IndicatorName,
    Code,
    Args,
    Output,
    EvaluatedAt,
}
