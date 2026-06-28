use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

const STATUSES: [&str; 4] = ["unverified", "supported", "refuted", "obsolete"];

#[derive(DeriveIden)]
#[allow(clippy::enum_variant_names)]
enum Hypothesis {
    Table,
    HypothesisId,
    StrategyId,
    Title,
    Body,
    Status,
    RelatedNoteIds,
    RelatedInterestIds,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum Strategy {
    Table,
    Id,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Hypothesis::Table)
                    .col(
                        ColumnDef::new(Hypothesis::HypothesisId)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Hypothesis::StrategyId).uuid().not_null())
                    .col(ColumnDef::new(Hypothesis::Title).text().not_null())
                    .col(ColumnDef::new(Hypothesis::Body).text().not_null())
                    .col(
                        ColumnDef::new(Hypothesis::Status)
                            .text()
                            .not_null()
                            .default("unverified"),
                    )
                    // related_note_ids / related_interest_ids は spec で uuid[] として確定済み。
                    // FK は張れない (note は単一テーブルだが strategy_interest は複合 PK のため
                    // UUID 単独では参照できない) 点に注意。整合性はアプリ層で担保する。
                    .col(
                        ColumnDef::new(Hypothesis::RelatedNoteIds)
                            .array(ColumnType::Uuid)
                            .not_null()
                            .default(Expr::cust("'{}'::uuid[]")),
                    )
                    .col(
                        ColumnDef::new(Hypothesis::RelatedInterestIds)
                            .array(ColumnType::Uuid)
                            .not_null()
                            .default(Expr::cust("'{}'::uuid[]")),
                    )
                    .col(
                        ColumnDef::new(Hypothesis::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(Hypothesis::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(Hypothesis::Table, Hypothesis::StrategyId)
                            .to(Strategy::Table, Strategy::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .check(Expr::col(Hypothesis::Status).is_in(STATUSES))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_hypothesis_strategy_updated")
                    .table(Hypothesis::Table)
                    .col(Hypothesis::StrategyId)
                    .col((Hypothesis::UpdatedAt, IndexOrder::Desc))
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Hypothesis::Table).to_owned())
            .await?;
        Ok(())
    }
}
