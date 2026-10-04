use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(DeriveIden)]
enum StrategyEarningsTarget {
    Table,
    StrategyId,
    RefKind,
    RefId,
    CreatedAt,
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
                    .table(StrategyEarningsTarget::Table)
                    .col(
                        ColumnDef::new(StrategyEarningsTarget::StrategyId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(StrategyEarningsTarget::RefKind)
                            .text()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(StrategyEarningsTarget::RefId)
                            .text()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(StrategyEarningsTarget::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .primary_key(
                        Index::create()
                            .col(StrategyEarningsTarget::StrategyId)
                            .col(StrategyEarningsTarget::RefKind)
                            .col(StrategyEarningsTarget::RefId),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_strategy_earnings_target_strategy")
                            .from(
                                StrategyEarningsTarget::Table,
                                StrategyEarningsTarget::StrategyId,
                            )
                            .to(Strategy::Table, Strategy::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .check(Expr::col(StrategyEarningsTarget::RefKind).is_in(["stock", "group"]))
                    .to_owned(),
            )
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(StrategyEarningsTarget::Table).to_owned())
            .await?;
        Ok(())
    }
}
