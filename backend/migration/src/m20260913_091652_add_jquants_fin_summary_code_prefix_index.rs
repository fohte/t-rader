use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20260913_091652_add_jquants_fin_summary_code_prefix_index"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // read_fin_summary は 4 桁銘柄コードを LEFT(code, 4) で突き合わせる。SeaQuery DSL は
        // 式インデックスを表現できないため raw SQL を使う。既存の (code, disc_no) PK は
        // 関数呼び出しに対しては使えず、この index が無いと毎回全表スキャンになる。
        manager
            .get_connection()
            .execute_unprepared(
                "CREATE INDEX idx_jquants_fin_summary_code_prefix \
                 ON jquants_fin_summary (LEFT(code, 4))",
            )
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("DROP INDEX idx_jquants_fin_summary_code_prefix")
            .await?;
        Ok(())
    }
}
