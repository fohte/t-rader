use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
                "ALTER TABLE annotation DROP CONSTRAINT annotation_target_kind_check",
            )
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
                "ALTER TABLE annotation \
                 ADD CONSTRAINT annotation_target_kind_check \
                 CHECK (target_kind IN ('signal', 'level', 'observation', 'other'))",
            )
            .await?;
        Ok(())
    }
}
