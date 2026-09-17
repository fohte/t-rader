use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20260915_170728_add_prediction_grade"
    }
}

#[derive(DeriveIden)]
enum PredictionGrade {
    Table,
    PredictionId,
    TargetBaseClose,
    TargetDueClose,
    BenchmarkBaseClose,
    BenchmarkDueClose,
    TargetReturn,
    BenchmarkReturn,
    Correct,
    GradedAt,
}

#[derive(DeriveIden)]
enum Prediction {
    Table,
    PredictionId,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(PredictionGrade::Table)
                    .col(
                        ColumnDef::new(PredictionGrade::PredictionId)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(PredictionGrade::TargetBaseClose)
                            .decimal()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PredictionGrade::TargetDueClose)
                            .decimal()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PredictionGrade::BenchmarkBaseClose)
                            .decimal()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PredictionGrade::BenchmarkDueClose)
                            .decimal()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PredictionGrade::TargetReturn)
                            .decimal()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PredictionGrade::BenchmarkReturn)
                            .decimal()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PredictionGrade::Correct)
                            .boolean()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PredictionGrade::GradedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(PredictionGrade::Table, PredictionGrade::PredictionId)
                            .to(Prediction::Table, Prediction::PredictionId)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(PredictionGrade::Table).to_owned())
            .await
    }
}
