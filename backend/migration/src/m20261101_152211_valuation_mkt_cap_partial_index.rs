use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
                "CREATE INDEX idx_valuation_code_prefix_mkt_cap_date \
                 ON valuation (LEFT(code, 4), date DESC) \
                 WHERE mkt_cap IS NOT NULL",
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("DROP INDEX idx_valuation_code_prefix_mkt_cap_date")
            .await?;

        Ok(())
    }
}
