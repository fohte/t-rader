use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(DeriveIden)]
enum Strategy {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum CustomIndicator {
    Table,
    IndicatorId,
    Name,
    Scope,
    StrategyId,
    Code,
    InputSchema,
    OutputSchema,
    Description,
    CreatedAt,
    UpdatedAt,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(CustomIndicator::Table)
                    .col(
                        ColumnDef::new(CustomIndicator::IndicatorId)
                            .uuid()
                            .not_null()
                            .primary_key()
                            .default(Expr::cust("gen_random_uuid()")),
                    )
                    .col(ColumnDef::new(CustomIndicator::Name).text().not_null())
                    .col(ColumnDef::new(CustomIndicator::Scope).text().not_null())
                    .col(ColumnDef::new(CustomIndicator::StrategyId).uuid())
                    .col(ColumnDef::new(CustomIndicator::Code).text().not_null())
                    .col(
                        ColumnDef::new(CustomIndicator::InputSchema)
                            .json_binary()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(CustomIndicator::OutputSchema)
                            .json_binary()
                            .not_null(),
                    )
                    .col(ColumnDef::new(CustomIndicator::Description).text())
                    .col(
                        ColumnDef::new(CustomIndicator::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(CustomIndicator::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(CustomIndicator::Table, CustomIndicator::StrategyId)
                            .to(Strategy::Table, Strategy::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // scope の値域と strategy_id の nullability 整合性は SeaQuery DSL では表現できないため raw SQL
        manager
            .get_connection()
            .execute_unprepared(
                "ALTER TABLE custom_indicator \
                 ADD CONSTRAINT custom_indicator_scope_check \
                 CHECK (scope IN ('global', 'strategy'))",
            )
            .await?;

        manager
            .get_connection()
            .execute_unprepared(
                "ALTER TABLE custom_indicator \
                 ADD CONSTRAINT custom_indicator_scope_strategy_id_check \
                 CHECK ((scope = 'global' AND strategy_id IS NULL) \
                     OR (scope = 'strategy' AND strategy_id IS NOT NULL))",
            )
            .await?;

        // 同一 scope 内でのみ name を unique にしたいので部分 unique index を使う
        manager
            .get_connection()
            .execute_unprepared(
                "CREATE UNIQUE INDEX custom_indicator_global_name_idx \
                 ON custom_indicator (name) WHERE scope = 'global'",
            )
            .await?;

        manager
            .get_connection()
            .execute_unprepared(
                "CREATE UNIQUE INDEX custom_indicator_strategy_name_idx \
                 ON custom_indicator (strategy_id, name) WHERE scope = 'strategy'",
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(CustomIndicator::Table).to_owned())
            .await?;
        Ok(())
    }
}
