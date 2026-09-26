use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20260925_122811_rename_valuation_tables"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let connection = manager.get_connection();
        connection
            .execute_unprepared("ALTER TABLE jquants_valuation RENAME TO valuation")
            .await?;
        connection
            .execute_unprepared(
                "ALTER TABLE jquants_valuation_ingested_date RENAME TO valuation_ingested_date",
            )
            .await?;
        connection
            .execute_unprepared("ALTER INDEX jquants_valuation_pkey RENAME TO valuation_pkey")
            .await?;
        connection
            .execute_unprepared(
                "ALTER INDEX jquants_valuation_ingested_date_pkey \
                 RENAME TO valuation_ingested_date_pkey",
            )
            .await?;
        connection
            .execute_unprepared(
                "ALTER INDEX idx_jquants_valuation_code_prefix_date \
                 RENAME TO idx_valuation_code_prefix_date",
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let connection = manager.get_connection();
        connection
            .execute_unprepared(
                "ALTER INDEX idx_valuation_code_prefix_date \
                 RENAME TO idx_jquants_valuation_code_prefix_date",
            )
            .await?;
        connection
            .execute_unprepared(
                "ALTER INDEX valuation_ingested_date_pkey \
                 RENAME TO jquants_valuation_ingested_date_pkey",
            )
            .await?;
        connection
            .execute_unprepared("ALTER INDEX valuation_pkey RENAME TO jquants_valuation_pkey")
            .await?;
        connection
            .execute_unprepared(
                "ALTER TABLE valuation_ingested_date \
                 RENAME TO jquants_valuation_ingested_date",
            )
            .await?;
        connection
            .execute_unprepared("ALTER TABLE valuation RENAME TO jquants_valuation")
            .await?;

        Ok(())
    }
}
