use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20260922_122232_add_jquants_valuation"
    }
}

#[derive(DeriveIden)]
enum JQuantsValuation {
    #[sea_orm(iden = "jquants_valuation")]
    Table,
    Code,
    Date,
    Eps,
    FwdEps,
    Bps,
    Roe,
    FwdRoe,
    Per,
    FwdPer,
    Pbr,
    MktCap,
}

#[derive(DeriveIden)]
enum JQuantsValuationIngestedDate {
    #[sea_orm(iden = "jquants_valuation_ingested_date")]
    Table,
    Date,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(JQuantsValuation::Table)
                    .col(ColumnDef::new(JQuantsValuation::Code).string().not_null())
                    .col(ColumnDef::new(JQuantsValuation::Date).date().not_null())
                    .col(ColumnDef::new(JQuantsValuation::Eps).decimal())
                    .col(ColumnDef::new(JQuantsValuation::FwdEps).decimal())
                    .col(ColumnDef::new(JQuantsValuation::Bps).decimal())
                    .col(ColumnDef::new(JQuantsValuation::Roe).decimal())
                    .col(ColumnDef::new(JQuantsValuation::FwdRoe).decimal())
                    .col(ColumnDef::new(JQuantsValuation::Per).decimal())
                    .col(ColumnDef::new(JQuantsValuation::FwdPer).decimal())
                    .col(ColumnDef::new(JQuantsValuation::Pbr).decimal())
                    .col(ColumnDef::new(JQuantsValuation::MktCap).decimal())
                    .primary_key(
                        Index::create()
                            .col(JQuantsValuation::Code)
                            .col(JQuantsValuation::Date),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(JQuantsValuationIngestedDate::Table)
                    .col(
                        ColumnDef::new(JQuantsValuationIngestedDate::Date)
                            .date()
                            .not_null(),
                    )
                    .primary_key(Index::create().col(JQuantsValuationIngestedDate::Date))
                    .to_owned(),
            )
            .await?;

        // MCP は4桁コードの前方一致と期間で検索するため、関数インデックスを用意する。
        manager
            .get_connection()
            .execute_unprepared(
                "CREATE INDEX idx_jquants_valuation_code_prefix_date \
                 ON jquants_valuation (LEFT(code, 4), date DESC)",
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("DROP INDEX idx_jquants_valuation_code_prefix_date")
            .await?;
        manager
            .drop_table(
                Table::drop()
                    .table(JQuantsValuationIngestedDate::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(Table::drop().table(JQuantsValuation::Table).to_owned())
            .await
    }
}
