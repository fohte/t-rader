use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20260925_093011_drop_strategy_interest"
    }
}

#[derive(DeriveIden)]
enum StrategyInterest {
    Table,
    Id,
    StrategyId,
    RefKind,
    RefId,
    Role,
    Origin,
    CreatedAt,
    Status,
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
            .drop_table(Table::drop().table(StrategyInterest::Table).to_owned())
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(StrategyInterest::Table)
                    .col(
                        ColumnDef::new(StrategyInterest::Id)
                            .uuid()
                            .not_null()
                            .default(Expr::cust("gen_random_uuid()")),
                    )
                    .col(ColumnDef::new(StrategyInterest::StrategyId).uuid())
                    .col(
                        ColumnDef::new(StrategyInterest::RefKind)
                            .string()
                            .not_null(),
                    )
                    .col(ColumnDef::new(StrategyInterest::RefId).string().not_null())
                    .col(ColumnDef::new(StrategyInterest::Role).string().not_null())
                    .col(ColumnDef::new(StrategyInterest::Origin).string().not_null())
                    .col(
                        ColumnDef::new(StrategyInterest::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(StrategyInterest::Status)
                            .string()
                            .not_null()
                            .default("active"),
                    )
                    .primary_key(Index::create().col(StrategyInterest::Id))
                    .foreign_key(
                        ForeignKey::create()
                            .from(StrategyInterest::Table, StrategyInterest::StrategyId)
                            .to(Strategy::Table, Strategy::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .check(Expr::col(StrategyInterest::RefKind).is_in([
                        "stock",
                        "indicator",
                        "sector",
                        "theme",
                    ]))
                    .check(Expr::col(StrategyInterest::Role).is_in(["seed", "derived"]))
                    .check(Expr::col(StrategyInterest::Origin).is_in(["human", "llm"]))
                    .to_owned(),
            )
            .await?;

        manager
            .get_connection()
            .execute_unprepared(
                "ALTER TABLE strategy_interest \
                 ADD CONSTRAINT strategy_interest_status_check \
                 CHECK (status IN ('active', 'archived'))",
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_strategy_interest_ref_kind_id")
                    .table(StrategyInterest::Table)
                    .col(StrategyInterest::RefKind)
                    .col(StrategyInterest::RefId)
                    .to_owned(),
            )
            .await?;
        manager
            .get_connection()
            .execute_unprepared(
                "CREATE UNIQUE INDEX strategy_interest_scoped_unique_idx \
                 ON strategy_interest (strategy_id, ref_kind, ref_id) WHERE strategy_id IS NOT NULL",
            )
            .await?;
        manager
            .get_connection()
            .execute_unprepared(
                "CREATE UNIQUE INDEX strategy_interest_global_unique_idx \
                 ON strategy_interest (ref_kind, ref_id) WHERE strategy_id IS NULL",
            )
            .await?;

        Ok(())
    }
}
