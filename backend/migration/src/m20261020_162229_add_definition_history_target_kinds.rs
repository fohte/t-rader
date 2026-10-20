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
                    .table(Alias::new("change_history"))
                    .drop_constraint(Alias::new("change_history_target_kind_check"))
                    .to_owned(),
            )
            .await?;
        db.execute_unprepared(
            "ALTER TABLE change_history \
             ADD CONSTRAINT change_history_target_kind_check \
             CHECK (target_kind IN ('note', 'annotation', 'strategy', 'trade', 'comment', 'custom_indicator', 'note_kind', 'stock_group', 'stock', 'agent_config', 'trigger'))",
        )
        .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        manager
            .alter_table(
                Table::alter()
                    .table(Alias::new("change_history"))
                    .drop_constraint(Alias::new("change_history_target_kind_check"))
                    .to_owned(),
            )
            .await?;
        // ロールバック後も追加種別の履歴を保持するため、既存行を再検証しない。
        db.execute_unprepared(
            "ALTER TABLE change_history \
             ADD CONSTRAINT change_history_target_kind_check \
             CHECK (target_kind IN ('note', 'annotation', 'strategy', 'trade', 'comment', 'custom_indicator', 'note_kind', 'stock_group', 'stock')) NOT VALID",
        )
        .await?;

        Ok(())
    }
}
