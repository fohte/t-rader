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
        // read_margin は 4 桁銘柄コードを LEFT(code, 4) で突き合わせる。既存の (code, date) /
        // (code, pub_date) インデックスは関数呼び出しに対しては使えず、この index が無いと
        // 毎回全表スキャンになる (m20260913_091652_add_jquants_fin_summary_code_prefix_index
        // と同じ理由)。
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
