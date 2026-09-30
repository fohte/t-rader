use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20260929_183001_note_version_superseded_status"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        db.execute_unprepared("ALTER TABLE note_version DROP CONSTRAINT note_version_status_check")
            .await?;
        db.execute_unprepared(
            "ALTER TABLE note_version ADD CONSTRAINT note_version_status_check CHECK (status IN ('approved', 'unread', 'rejected', 'superseded'))",
        )
        .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        db.execute_unprepared("ALTER TABLE note_version DROP CONSTRAINT note_version_status_check")
            .await?;
        db.execute_unprepared(
            "UPDATE note_version SET status = 'unread' WHERE status = 'superseded'",
        )
        .await?;
        db.execute_unprepared(
            "ALTER TABLE note_version ADD CONSTRAINT note_version_status_check CHECK (status IN ('approved', 'unread', 'rejected'))",
        )
        .await?;
        Ok(())
    }
}
