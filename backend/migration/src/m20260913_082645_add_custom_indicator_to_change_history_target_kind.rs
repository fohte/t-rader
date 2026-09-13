use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

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
                 CHECK (target_kind IN ('note', 'annotation', 'strategy', 'trade', 'comment', 'custom_indicator'))",
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
        manager
            .get_connection()
            .execute_unprepared(
                "ALTER TABLE change_history \
                 ADD CONSTRAINT change_history_target_kind_check \
                 CHECK (target_kind IN ('note', 'annotation', 'strategy', 'trade', 'comment'))",
            )
            .await?;
        Ok(())
    }
}
