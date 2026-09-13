use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20260913_133130_add_margin_code_prefix_indexes"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // LEFT(code, 4) 検索用。既存の (code, date) / (code, pub_date) インデックスは
        // 関数呼び出しに対しては使えないため式インデックスを張る。
        manager
            .get_connection()
            .execute_unprepared(
                "CREATE INDEX idx_margin_interest_code_prefix \
                 ON margin_interest (LEFT(code, 4))",
            )
            .await?;
        manager
            .get_connection()
            .execute_unprepared(
                "CREATE INDEX idx_margin_alert_code_prefix \
                 ON margin_alert (LEFT(code, 4))",
            )
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("DROP INDEX idx_margin_alert_code_prefix")
            .await?;
        manager
            .get_connection()
            .execute_unprepared("DROP INDEX idx_margin_interest_code_prefix")
            .await?;
        Ok(())
    }
}
