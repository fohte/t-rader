use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20260913_125042_add_prediction"
    }
}

const DIRECTIONS: [&str; 2] = ["outperform", "underperform"];
const PROBABILITY_STEPS: [f64; 8] = [0.55, 0.60, 0.65, 0.70, 0.75, 0.80, 0.85, 0.90];

#[derive(DeriveIden)]
#[allow(clippy::enum_variant_names)]
enum Prediction {
    Table,
    PredictionId,
    StrategyId,
    NoteId,
    TargetStockId,
    BenchmarkStockId,
    Direction,
    Probability,
    BaseDate,
    DueDate,
    CreatedAt,
}

#[derive(DeriveIden)]
enum Strategy {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum Note {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum Stock {
    Table,
    Id,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Prediction::Table)
                    .col(
                        ColumnDef::new(Prediction::PredictionId)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Prediction::StrategyId).uuid().not_null())
                    .col(ColumnDef::new(Prediction::NoteId).uuid())
                    .col(
                        ColumnDef::new(Prediction::TargetStockId)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Prediction::BenchmarkStockId)
                            .string()
                            .not_null(),
                    )
                    .col(ColumnDef::new(Prediction::Direction).text().not_null())
                    .col(ColumnDef::new(Prediction::Probability).decimal().not_null())
                    .col(ColumnDef::new(Prediction::BaseDate).date().not_null())
                    .col(ColumnDef::new(Prediction::DueDate).date().not_null())
                    .col(
                        ColumnDef::new(Prediction::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(Prediction::Table, Prediction::StrategyId)
                            .to(Strategy::Table, Strategy::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(Prediction::Table, Prediction::NoteId)
                            .to(Note::Table, Note::Id)
                            .on_delete(ForeignKeyAction::SetNull),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(Prediction::Table, Prediction::TargetStockId)
                            .to(Stock::Table, Stock::Id),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(Prediction::Table, Prediction::BenchmarkStockId)
                            .to(Stock::Table, Stock::Id),
                    )
                    .check(Expr::col(Prediction::Direction).is_in(DIRECTIONS))
                    .check(Expr::col(Prediction::Probability).is_in(PROBABILITY_STEPS))
                    // 対象と比較対象が同一銘柄だと「上回る/下回る」の判定が成立しない
                    .check(
                        Expr::col(Prediction::TargetStockId)
                            .ne(Expr::col(Prediction::BenchmarkStockId)),
                    )
                    .check(Expr::col(Prediction::DueDate).gt(Expr::col(Prediction::BaseDate)))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_prediction_strategy_due")
                    .table(Prediction::Table)
                    .col(Prediction::StrategyId)
                    .col(Prediction::DueDate)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Prediction::Table).to_owned())
            .await?;
        Ok(())
    }
}
