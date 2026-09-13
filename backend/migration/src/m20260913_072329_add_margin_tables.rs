use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

/// margin_interest テーブルのカラム識別子 (信用取引週末残高、2026-09-28 以降は信用取引残高)
#[derive(DeriveIden)]
enum MarginInterest {
    Table,
    Date,
    Code,
    IssType,
    ShrtVol,
    LongVol,
    ShrtNegVol,
    LongNegVol,
    ShrtStdVol,
    LongStdVol,
    ShrtVal,
    LongVal,
    ShrtNegVal,
    LongNegVal,
    ShrtStdVal,
    LongStdVal,
}

/// margin_alert テーブルのカラム識別子 (日々公表信用取引残高)
#[derive(DeriveIden)]
enum MarginAlert {
    Table,
    PubDate,
    Code,
    AppDate,
    PubReason,
    ShrtOut,
    LongOut,
    ShrtOutChg,
    LongOutChg,
    ShrtOutRatio,
    LongOutRatio,
    SlRatio,
    ShrtNegOut,
    ShrtStdOut,
    LongNegOut,
    LongStdOut,
    TseMrgnRegCls,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // margin_interest テーブル。J-Quants の Code (5 桁) をそのまま保存し、
        // stock/instruments への外部キーは張らない (4 桁/5 桁の突き合わせは読む側の責務)。
        // Val 系カラムは新仕様 (2026-09-28 切替、2026-09-25 申込分以降) の金額項目で、
        // 切替前の日付では null。切替に備えてテーブルは最初から持っておく。
        manager
            .create_table(
                Table::create()
                    .table(MarginInterest::Table)
                    .col(ColumnDef::new(MarginInterest::Date).date().not_null())
                    .col(ColumnDef::new(MarginInterest::Code).string().not_null())
                    .col(
                        ColumnDef::new(MarginInterest::IssType)
                            .small_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MarginInterest::ShrtVol)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MarginInterest::LongVol)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MarginInterest::ShrtNegVol)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MarginInterest::LongNegVol)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MarginInterest::ShrtStdVol)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MarginInterest::LongStdVol)
                            .big_integer()
                            .not_null(),
                    )
                    .col(ColumnDef::new(MarginInterest::ShrtVal).big_integer())
                    .col(ColumnDef::new(MarginInterest::LongVal).big_integer())
                    .col(ColumnDef::new(MarginInterest::ShrtNegVal).big_integer())
                    .col(ColumnDef::new(MarginInterest::LongNegVal).big_integer())
                    .col(ColumnDef::new(MarginInterest::ShrtStdVal).big_integer())
                    .col(ColumnDef::new(MarginInterest::LongStdVal).big_integer())
                    .primary_key(
                        Index::create()
                            .col(MarginInterest::Date)
                            .col(MarginInterest::Code)
                            .col(MarginInterest::IssType),
                    )
                    .check(Expr::col(MarginInterest::IssType).is_in([1, 2, 3]))
                    .to_owned(),
            )
            .await?;

        // 銘柄単位の時系列参照 (「この銘柄の信用残の推移」) を素早く引けるようにする。
        // PK は (date, code, iss_type) の順のため、date を指定しない code 単体の絞り込みには効かない。
        manager
            .create_index(
                Index::create()
                    .name("idx_margin_interest_code_date")
                    .table(MarginInterest::Table)
                    .col(MarginInterest::Code)
                    .col(MarginInterest::Date)
                    .to_owned(),
            )
            .await?;

        // margin_alert テーブル。訂正は上書きではなく、同じ AppDate で PubDate が新しい行として
        // 追加される。PubDate を PK に含めないと訂正前後のどちらかが上書きで消えるため、
        // (pub_date, code) を PK にする。
        manager
            .create_table(
                Table::create()
                    .table(MarginAlert::Table)
                    .col(ColumnDef::new(MarginAlert::PubDate).date().not_null())
                    .col(ColumnDef::new(MarginAlert::Code).string().not_null())
                    .col(ColumnDef::new(MarginAlert::AppDate).date().not_null())
                    .col(
                        ColumnDef::new(MarginAlert::PubReason)
                            .json_binary()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MarginAlert::ShrtOut)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MarginAlert::LongOut)
                            .big_integer()
                            .not_null(),
                    )
                    .col(ColumnDef::new(MarginAlert::ShrtOutChg).big_integer())
                    .col(ColumnDef::new(MarginAlert::LongOutChg).big_integer())
                    .col(ColumnDef::new(MarginAlert::ShrtOutRatio).decimal())
                    .col(ColumnDef::new(MarginAlert::LongOutRatio).decimal())
                    .col(ColumnDef::new(MarginAlert::SlRatio).decimal())
                    .col(
                        ColumnDef::new(MarginAlert::ShrtNegOut)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MarginAlert::ShrtStdOut)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MarginAlert::LongNegOut)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MarginAlert::LongStdOut)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MarginAlert::TseMrgnRegCls)
                            .string()
                            .not_null(),
                    )
                    .primary_key(
                        Index::create()
                            .col(MarginAlert::PubDate)
                            .col(MarginAlert::Code),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_margin_alert_code_pub_date")
                    .table(MarginAlert::Table)
                    .col(MarginAlert::Code)
                    .col(MarginAlert::PubDate)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(MarginAlert::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(MarginInterest::Table).to_owned())
            .await?;
        Ok(())
    }
}
