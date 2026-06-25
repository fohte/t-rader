use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(DeriveIden)]
enum Strategy {
    Table,
    Id,
}

#[derive(DeriveIden)]
#[allow(clippy::enum_variant_names)]
enum Trigger {
    Table,
    TriggerId,
    StrategyId,
    Kind,
    Schedule,
    HookSlug,
    EventMatch,
    PromptTemplate,
    Enabled,
    LastFiredAt,
    CreatedAt,
    UpdatedAt,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Trigger::Table)
                    .col(
                        ColumnDef::new(Trigger::TriggerId)
                            .uuid()
                            .not_null()
                            .primary_key()
                            .default(Expr::cust("gen_random_uuid()")),
                    )
                    .col(ColumnDef::new(Trigger::StrategyId).uuid().not_null())
                    .col(ColumnDef::new(Trigger::Kind).text().not_null())
                    .col(ColumnDef::new(Trigger::Schedule).text())
                    .col(ColumnDef::new(Trigger::HookSlug).text())
                    .col(ColumnDef::new(Trigger::EventMatch).json_binary())
                    .col(ColumnDef::new(Trigger::PromptTemplate).text().not_null())
                    .col(
                        ColumnDef::new(Trigger::Enabled)
                            .boolean()
                            .not_null()
                            .default(true),
                    )
                    .col(ColumnDef::new(Trigger::LastFiredAt).timestamp_with_time_zone())
                    .col(
                        ColumnDef::new(Trigger::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(Trigger::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(Trigger::Table, Trigger::StrategyId)
                            .to(Strategy::Table, Strategy::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // kind 値域と (kind, schedule, hook_slug) の組合せを DB レベルで強制する
        manager
            .get_connection()
            .execute_unprepared(
                "ALTER TABLE \"trigger\" ADD CONSTRAINT trigger_kind_shape_check CHECK (\
                    (kind = 'cron' AND schedule IS NOT NULL AND hook_slug IS NULL) \
                    OR (kind = 'hook' AND hook_slug IS NOT NULL AND schedule IS NULL)\
                )",
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("trigger_strategy_idx")
                    .table(Trigger::Table)
                    .col(Trigger::StrategyId)
                    .col((Trigger::CreatedAt, IndexOrder::Desc))
                    .to_owned(),
            )
            .await?;

        manager
            .get_connection()
            .execute_unprepared(
                "CREATE UNIQUE INDEX trigger_hook_slug_idx \
                 ON \"trigger\" (hook_slug) WHERE hook_slug IS NOT NULL",
            )
            .await?;

        manager
            .get_connection()
            .execute_unprepared(
                "CREATE INDEX trigger_cron_enabled_idx \
                 ON \"trigger\" (enabled, last_fired_at) WHERE kind = 'cron'",
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Trigger::Table).to_owned())
            .await?;
        Ok(())
    }
}
