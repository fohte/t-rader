use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        manager
            .alter_table(
                Table::alter()
                    .table(Alias::new("instruments"))
                    .drop_constraint(Alias::new("instruments_market_check"))
                    .to_owned(),
            )
            .await?;
        // SeaQuery の ALTER TABLE API は ADD CHECK を扱えないため、制約式の追加に SQL を使う。
        db.execute_unprepared(
            "ALTER TABLE instruments \
             ADD CONSTRAINT instruments_market_check \
             CHECK (market IN ('TSE', 'US', 'OTHER'))",
        )
        .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Alias::new("change_history"))
                    .drop_constraint(Alias::new("change_history_target_kind_check"))
                    .to_owned(),
            )
            .await?;
        db.execute_unprepared(
            "ALTER TABLE change_history \
             ADD CONSTRAINT change_history_target_kind_check \
             CHECK (target_kind IN ('note', 'annotation', 'strategy', 'trade', 'comment', 'custom_indicator', 'note_kind', 'stock_group', 'stock'))",
        )
        .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        manager
            .alter_table(
                Table::alter()
                    .table(Alias::new("instruments"))
                    .drop_constraint(Alias::new("instruments_market_check"))
                    .to_owned(),
            )
            .await?;
        db.execute_unprepared(
            "ALTER TABLE instruments \
             ADD CONSTRAINT instruments_market_check \
             CHECK (market IN ('TSE')) NOT VALID",
        )
        .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Alias::new("change_history"))
                    .drop_constraint(Alias::new("change_history_target_kind_check"))
                    .to_owned(),
            )
            .await?;
        db.execute_unprepared(
            "ALTER TABLE change_history \
             ADD CONSTRAINT change_history_target_kind_check \
             CHECK (target_kind IN ('note', 'annotation', 'strategy', 'trade', 'comment', 'custom_indicator', 'note_kind', 'stock_group')) NOT VALID",
        )
        .await?;

        Ok(())
    }
}
