use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20260909_165140_make_strategy_interest_strategy_id_nullable"
    }
}

#[derive(DeriveIden)]
enum StrategyInterest {
    Table,
    Id,
    StrategyId,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(StrategyInterest::Table)
                    .add_column(
                        ColumnDef::new(StrategyInterest::Id)
                            .uuid()
                            .not_null()
                            .default(Expr::cust("gen_random_uuid()")),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .get_connection()
            .execute_unprepared(
                "ALTER TABLE strategy_interest DROP CONSTRAINT strategy_interest_pkey",
            )
            .await?;
        manager
            .get_connection()
            .execute_unprepared("ALTER TABLE strategy_interest ADD PRIMARY KEY (id)")
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(StrategyInterest::Table)
                    .modify_column(ColumnDef::new(StrategyInterest::StrategyId).uuid().null())
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

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("DROP INDEX strategy_interest_global_unique_idx")
            .await?;
        manager
            .get_connection()
            .execute_unprepared("DROP INDEX strategy_interest_scoped_unique_idx")
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(StrategyInterest::Table)
                    .modify_column(
                        ColumnDef::new(StrategyInterest::StrategyId)
                            .uuid()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .get_connection()
            .execute_unprepared(
                "ALTER TABLE strategy_interest DROP CONSTRAINT strategy_interest_pkey",
            )
            .await?;
        manager
            .get_connection()
            .execute_unprepared(
                "ALTER TABLE strategy_interest ADD PRIMARY KEY (strategy_id, ref_kind, ref_id)",
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(StrategyInterest::Table)
                    .drop_column(StrategyInterest::Id)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}
