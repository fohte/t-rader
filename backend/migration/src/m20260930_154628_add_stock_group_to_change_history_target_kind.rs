use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20260930_154628_add_stock_group_to_change_history_target_kind"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
                "ALTER TABLE change_history DROP CONSTRAINT change_history_target_kind_check",
            )
            .await?;
        manager
            .get_connection()
            .execute_unprepared(
                "ALTER TABLE change_history \
                 ADD CONSTRAINT change_history_target_kind_check \
                 CHECK (target_kind IN ('note', 'annotation', 'strategy', 'trade', 'comment', 'custom_indicator', 'note_kind', 'stock_group'))",
            )
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
                "ALTER TABLE change_history DROP CONSTRAINT change_history_target_kind_check",
            )
            .await?;
        // ロールバック後も既存のグループ履歴を残すため、制約を再検証しない。
        manager
            .get_connection()
            .execute_unprepared(
                "ALTER TABLE change_history \
                 ADD CONSTRAINT change_history_target_kind_check \
                 CHECK (target_kind IN ('note', 'annotation', 'strategy', 'trade', 'comment', 'custom_indicator', 'note_kind')) NOT VALID",
            )
            .await?;
        Ok(())
    }
}
