use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20260925_043733_note_version_review"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Comment::Table)
                    .add_column(ColumnDef::new(Comment::AnchorSide).text())
                    .drop_column(Comment::Drifted)
                    .to_owned(),
            )
            .await?;

        let db = manager.get_connection();
        db.execute_unprepared("ALTER TABLE comment DROP CONSTRAINT comment_target_kind_check")
            .await?;
        db.execute_unprepared(
            r#"
            UPDATE comment AS c
            SET
                target_kind = 'note_version',
                target_id = v.id,
                anchor_side = CASE WHEN c.start_line IS NOT NULL THEN 'new' ELSE NULL END
            FROM note_version AS v
            WHERE c.target_kind = 'note'
              AND c.target_id = v.note_id
              AND v.is_current;
            "#,
        )
        .await?;
        db.execute_unprepared(
            "ALTER TABLE comment ADD CONSTRAINT comment_target_kind_check CHECK (target_kind IN ('note', 'note_version', 'annotation'))",
        )
        .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        db.execute_unprepared("ALTER TABLE comment DROP CONSTRAINT comment_target_kind_check")
            .await?;
        db.execute_unprepared(
            r#"
            UPDATE comment AS c
            SET target_kind = 'note', target_id = v.note_id
            FROM note_version AS v
            WHERE c.target_kind = 'note_version'
              AND c.target_id = v.id;
            "#,
        )
        .await?;
        db.execute_unprepared(
            "UPDATE comment SET target_kind = 'note' WHERE target_kind = 'note_version'",
        )
        .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Comment::Table)
                    .drop_column(Comment::AnchorSide)
                    .add_column(
                        ColumnDef::new(Comment::Drifted)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .to_owned(),
            )
            .await?;
        db.execute_unprepared(
            "ALTER TABLE comment ADD CONSTRAINT comment_target_kind_check CHECK (target_kind IN ('note', 'annotation'))",
        )
        .await?;
        Ok(())
    }
}

#[derive(DeriveIden)]
enum Comment {
    Table,
    AnchorSide,
    Drifted,
}
