use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20261027_134424_calendar_event_reference_document_period"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
                "ALTER TABLE calendar_event_reference \
                 ADD CONSTRAINT calendar_event_reference_document_period_check \
                 CHECK (kind <> 'individual_document' OR (target_period_end IS NOT NULL AND target_period_type IS NOT NULL))",
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
                "ALTER TABLE calendar_event_reference \
                 DROP CONSTRAINT calendar_event_reference_document_period_check",
            )
            .await?;

        Ok(())
    }
}
