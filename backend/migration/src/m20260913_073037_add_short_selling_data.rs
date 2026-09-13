use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(DeriveIden)]
enum ShortSaleReport {
    Table,
    DiscDate,
    CalcDate,
    Code,
    SsName,
    SsAddr,
    DicName,
    DicAddr,
    FundName,
    ShortPositionRatio,
    ShortPositionShares,
    ShortPositionUnits,
    PrevReportDate,
    PrevReportRatio,
    Notes,
}

#[derive(DeriveIden)]
enum ShortRatio {
    Table,
    Date,
    Sector33Code,
    SellExcludingShortValue,
    ShortWithRestrictionValue,
    ShortWithoutRestrictionValue,
}

#[derive(DeriveIden)]
enum Idx {
    #[sea_orm(iden = "idx_short_sale_report_code_disc_date")]
    ShortSaleReportCodeDiscDate,
    #[sea_orm(iden = "idx_short_ratio_sector33_code_date")]
    ShortRatioSector33CodeDate,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // 空売り残高報告 (J-Quants `/v2/markets/short-sale-report`)。
        // 同一日・同一銘柄に報告者ごとの行が並び、報告者を識別するコードが無いため、
        // 報告者を特定する項目一式を PK に含める。
        manager
            .create_table(
                Table::create()
                    .table(ShortSaleReport::Table)
                    .col(ColumnDef::new(ShortSaleReport::DiscDate).date().not_null())
                    .col(ColumnDef::new(ShortSaleReport::CalcDate).date().not_null())
                    .col(ColumnDef::new(ShortSaleReport::Code).text().not_null())
                    .col(ColumnDef::new(ShortSaleReport::SsName).text().not_null())
                    .col(ColumnDef::new(ShortSaleReport::SsAddr).text().not_null())
                    .col(ColumnDef::new(ShortSaleReport::DicName).text().not_null())
                    .col(ColumnDef::new(ShortSaleReport::DicAddr).text().not_null())
                    .col(ColumnDef::new(ShortSaleReport::FundName).text().not_null())
                    .col(
                        ColumnDef::new(ShortSaleReport::ShortPositionRatio)
                            .decimal()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ShortSaleReport::ShortPositionShares)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ShortSaleReport::ShortPositionUnits)
                            .big_integer()
                            .not_null(),
                    )
                    .col(ColumnDef::new(ShortSaleReport::PrevReportDate).date())
                    .col(ColumnDef::new(ShortSaleReport::PrevReportRatio).decimal())
                    .col(ColumnDef::new(ShortSaleReport::Notes).text().not_null())
                    .primary_key(
                        Index::create()
                            .col(ShortSaleReport::DiscDate)
                            .col(ShortSaleReport::Code)
                            .col(ShortSaleReport::SsName)
                            .col(ShortSaleReport::SsAddr)
                            .col(ShortSaleReport::DicName)
                            .col(ShortSaleReport::DicAddr)
                            .col(ShortSaleReport::FundName),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name(Idx::ShortSaleReportCodeDiscDate.to_string())
                    .table(ShortSaleReport::Table)
                    .col(ShortSaleReport::Code)
                    .col(ShortSaleReport::DiscDate)
                    .to_owned(),
            )
            .await?;

        // 業種別空売り比率 (J-Quants `/v2/markets/short-ratio`)。33 業種コードとの
        // 対応付けは読む側に委ねるため、S33 は文字列のまま保持する。
        manager
            .create_table(
                Table::create()
                    .table(ShortRatio::Table)
                    .col(ColumnDef::new(ShortRatio::Date).date().not_null())
                    .col(ColumnDef::new(ShortRatio::Sector33Code).text().not_null())
                    .col(ColumnDef::new(ShortRatio::SellExcludingShortValue).decimal())
                    .col(ColumnDef::new(ShortRatio::ShortWithRestrictionValue).decimal())
                    .col(ColumnDef::new(ShortRatio::ShortWithoutRestrictionValue).decimal())
                    .primary_key(
                        Index::create()
                            .col(ShortRatio::Date)
                            .col(ShortRatio::Sector33Code),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name(Idx::ShortRatioSector33CodeDate.to_string())
                    .table(ShortRatio::Table)
                    .col(ShortRatio::Sector33Code)
                    .col(ShortRatio::Date)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(ShortRatio::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(ShortSaleReport::Table).to_owned())
            .await
    }
}
