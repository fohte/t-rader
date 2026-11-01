use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(DeriveIden)]
enum NoteTag {
    #[sea_orm(iden = "note_tag")]
    Table,
    Key,
    DisplayName,
    Description,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(NoteTag::Table)
                    .col(ColumnDef::new(NoteTag::Key).text().not_null())
                    .col(ColumnDef::new(NoteTag::DisplayName).text().not_null())
                    .col(ColumnDef::new(NoteTag::Description).text())
                    .primary_key(Index::create().col(NoteTag::Key))
                    .to_owned(),
            )
            .await?;

        manager
            .get_connection()
            .execute_unprepared(
                "ALTER TABLE note_tag ADD CONSTRAINT note_tag_key_nonblank_check CHECK (key ~ '[^[:space:]]')",
            )
            .await?;
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
                 CHECK (target_kind IN ('note', 'annotation', 'strategy', 'trade', 'comment', 'custom_indicator', 'note_kind', 'stock_group', 'stock', 'agent_config', 'trigger', 'note_tag'))",
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
                 CHECK (target_kind IN ('note', 'annotation', 'strategy', 'trade', 'comment', 'custom_indicator', 'note_kind', 'stock_group', 'stock', 'agent_config', 'trigger')) NOT VALID",
            )
            .await?;

        manager
            .drop_table(Table::drop().table(NoteTag::Table).to_owned())
            .await?;

        Ok(())
    }
}
