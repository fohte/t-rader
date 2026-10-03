use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        db.execute_unprepared("ALTER TABLE instruments DROP CONSTRAINT instruments_market_check")
            .await?;
        db.execute_unprepared(
            "ALTER TABLE instruments \
             ADD CONSTRAINT instruments_market_check \
             CHECK (market IN ('TSE', 'US', 'OTHER'))",
        )
        .await?;

        db.execute_unprepared(
            "ALTER TABLE change_history DROP CONSTRAINT change_history_target_kind_check",
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
        db.execute_unprepared("ALTER TABLE instruments DROP CONSTRAINT instruments_market_check")
            .await?;
        db.execute_unprepared(
            "ALTER TABLE instruments \
             ADD CONSTRAINT instruments_market_check \
             CHECK (market IN ('TSE')) NOT VALID",
        )
        .await?;

        db.execute_unprepared(
            "ALTER TABLE change_history DROP CONSTRAINT change_history_target_kind_check",
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
