use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(DeriveIden)]
enum NoteKind {
    #[sea_orm(iden = "note_kind")]
    Table,
    Key,
    DisplayName,
    RequiresApproval,
    Description,
    SortOrder,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(NoteKind::Table)
                    .col(ColumnDef::new(NoteKind::Key).text().not_null())
                    .col(ColumnDef::new(NoteKind::DisplayName).text().not_null())
                    .col(
                        ColumnDef::new(NoteKind::RequiresApproval)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .col(ColumnDef::new(NoteKind::Description).text())
                    .col(
                        ColumnDef::new(NoteKind::SortOrder)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .primary_key(Index::create().col(NoteKind::Key))
                    .to_owned(),
            )
            .await?;

        manager
            .get_connection()
            .execute_unprepared(
                "INSERT INTO note_kind (key, display_name, requires_approval, description, sort_order) \
                 SELECT DISTINCT type_tag, type_tag, FALSE, NULL, 0 \
                 FROM note WHERE type_tag IS NOT NULL",
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
                 CHECK (target_kind IN ('note', 'annotation', 'strategy', 'trade', 'comment', 'custom_indicator', 'note_kind'))",
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
        // 操作履歴を保持したまま旧コードへ戻せるよう、既存行は再検証しない。
        manager
            .get_connection()
            .execute_unprepared(
                "ALTER TABLE change_history \
                 ADD CONSTRAINT change_history_target_kind_check \
                 CHECK (target_kind IN ('note', 'annotation', 'strategy', 'trade', 'comment', 'custom_indicator')) NOT VALID",
            )
            .await?;

        manager
            .drop_table(Table::drop().table(NoteKind::Table).to_owned())
            .await
    }
}
