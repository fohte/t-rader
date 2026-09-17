use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20260915_174024_add_calc_date_to_short_sale_report_pk"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // 同一 disc_date (公表日) に複数の calc_date (計算日) の報告が公表されるケースが
        // あり、calc_date を含めないと別報告が同じ主キーとして衝突してしまうため追加する。
        manager
            .get_connection()
            .execute_unprepared(
                "ALTER TABLE short_sale_report DROP CONSTRAINT short_sale_report_pkey",
            )
            .await?;
        manager
            .get_connection()
            .execute_unprepared(
                "ALTER TABLE short_sale_report ADD PRIMARY KEY \
                 (disc_date, calc_date, code, ss_name, ss_addr, dic_name, dic_addr, fund_name)",
            )
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
                "ALTER TABLE short_sale_report DROP CONSTRAINT short_sale_report_pkey",
            )
            .await?;
        manager
            .get_connection()
            .execute_unprepared(
                "ALTER TABLE short_sale_report ADD PRIMARY KEY \
                 (disc_date, code, ss_name, ss_addr, dic_name, dic_addr, fund_name)",
            )
            .await?;
        Ok(())
    }
}
