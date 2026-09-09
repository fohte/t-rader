use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(DeriveIden)]
enum StrategyInvestableAmount {
    Table,
    Id,
    StrategyId,
    AmountJpy,
    EffectiveAt,
    CreatedAt,
}

#[derive(DeriveIden)]
enum Strategy {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum PortfolioSnapshot {
    Table,
    Id,
    TakenAt,
    CashJpy,
    TotalEquityJpy,
    PositionsJson,
    CreatedAt,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(StrategyInvestableAmount::Table)
                    .col(
                        ColumnDef::new(StrategyInvestableAmount::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(StrategyInvestableAmount::StrategyId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(StrategyInvestableAmount::AmountJpy)
                            .decimal()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(StrategyInvestableAmount::EffectiveAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(StrategyInvestableAmount::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(
                                StrategyInvestableAmount::Table,
                                StrategyInvestableAmount::StrategyId,
                            )
                            .to(Strategy::Table, Strategy::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // 「effective_at が現在時刻以下の最新行」を引く検索の絞り込みに使う。
        manager
            .create_index(
                Index::create()
                    .name("idx_strategy_investable_amount_strategy_effective")
                    .table(StrategyInvestableAmount::Table)
                    .col(StrategyInvestableAmount::StrategyId)
                    .col((StrategyInvestableAmount::EffectiveAt, IndexOrder::Desc))
                    .to_owned(),
            )
            .await?;

        // 参照元コードが無いまま本番 0 件で放置されていたテーブルを削除する。
        // total_equity_jpy は Σ(qty × 現在値) + 未使用枠 から導出する方針のため、
        // 導出可能な値を別途保持する portfolio_snapshot は復活させない。
        manager
            .drop_table(Table::drop().table(PortfolioSnapshot::Table).to_owned())
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(PortfolioSnapshot::Table)
                    .col(
                        ColumnDef::new(PortfolioSnapshot::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(PortfolioSnapshot::TakenAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PortfolioSnapshot::CashJpy)
                            .decimal()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PortfolioSnapshot::TotalEquityJpy)
                            .decimal()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PortfolioSnapshot::PositionsJson)
                            .json_binary()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PortfolioSnapshot::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_portfolio_snapshot_taken_at")
                    .table(PortfolioSnapshot::Table)
                    .col(PortfolioSnapshot::TakenAt)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_table(
                Table::drop()
                    .table(StrategyInvestableAmount::Table)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}
