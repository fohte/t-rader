use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20260925_115400_add_note_kind_fk_and_nonblank_checks"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
                "UPDATE note SET type_tag = NULL \
                 WHERE type_tag IS NOT NULL AND type_tag !~ '[^[:space:]]'",
            )
            .await?;
        manager
            .get_connection()
            .execute_unprepared("DELETE FROM note_kind WHERE key !~ '[^[:space:]]'")
            .await?;
        manager
            .get_connection()
            .execute_unprepared("ALTER TABLE note RENAME COLUMN type_tag TO kind")
            .await?;
        manager
            .get_connection()
            .execute_unprepared(
                "ALTER TABLE note_kind \
                 ADD CONSTRAINT note_kind_key_nonblank_check CHECK (key ~ '[^[:space:]]')",
            )
            .await?;
        manager
            .get_connection()
            .execute_unprepared(
                "ALTER TABLE note \
                 ADD CONSTRAINT note_kind_nonblank_check CHECK (kind IS NULL OR kind ~ '[^[:space:]]')",
            )
            .await?;
        manager
            .get_connection()
            .execute_unprepared(
                "ALTER TABLE note \
                 ADD CONSTRAINT fk_note_kind FOREIGN KEY (kind) REFERENCES note_kind (key) ON DELETE RESTRICT",
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("ALTER TABLE note DROP CONSTRAINT fk_note_kind")
            .await?;
        manager
            .get_connection()
            .execute_unprepared("ALTER TABLE note DROP CONSTRAINT note_kind_nonblank_check")
            .await?;
        manager
            .get_connection()
            .execute_unprepared(
                "ALTER TABLE note_kind DROP CONSTRAINT note_kind_key_nonblank_check",
            )
            .await?;
        manager
            .get_connection()
            .execute_unprepared("ALTER TABLE note RENAME COLUMN kind TO type_tag")
            .await?;

        Ok(())
    }
}
