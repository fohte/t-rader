use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20261021_162308_execution_payload_capture_metadata"
    }
}

#[derive(DeriveIden)]
enum TaskExecutionPayload {
    Table,
    CaptureMetadata,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(TaskExecutionPayload::Table)
                    .add_column(ColumnDef::new(TaskExecutionPayload::CaptureMetadata).json_binary())
                    .to_owned(),
            )
            .await?;
        manager
            .get_connection()
            .execute_unprepared(
                "ALTER TABLE task_execution_payload \
                 ADD CONSTRAINT task_execution_payload_capture_metadata_object_check \
                 CHECK (capture_metadata IS NULL OR jsonb_typeof(capture_metadata) = 'object')",
            )
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
                "ALTER TABLE task_execution_payload \
                 DROP CONSTRAINT task_execution_payload_capture_metadata_object_check",
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(TaskExecutionPayload::Table)
                    .drop_column(TaskExecutionPayload::CaptureMetadata)
                    .to_owned(),
            )
            .await?;
        Ok(())
    }
}
