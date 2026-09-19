use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // 保証範囲を列定義の読み手にも伝えるため、DB 側にも書いておく。
        manager
            .get_connection()
            .execute_unprepared(
                "COMMENT ON COLUMN strategy_task.as_of IS \
                 '実行の論理的な基準時刻。投入時に決まり、resume でも変わらない。\
                 監査用に記録し、agent がプロンプトにも含めて LLM に伝える。実行中に参照したデータがすべてこの時刻のものであることは保証しない \
                 (データ取得層は基準時刻を受け取らず、呼び出された瞬間の外部データを返す)。'",
            )
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("COMMENT ON COLUMN strategy_task.as_of IS NULL")
            .await?;
        Ok(())
    }
}
