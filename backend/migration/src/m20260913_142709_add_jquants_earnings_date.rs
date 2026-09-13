use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20260913_142709_add_jquants_earnings_date"
    }
}

#[derive(DeriveIden)]
enum JQuantsEarningsDate {
    // DeriveIden の既定の snake_case 変換は連続大文字 "JQ" を "j_q" と別語扱いしてしまう
    // (`j_quants_earnings_date` になる) ため、テーブル名は明示的に上書きする。
    #[sea_orm(iden = "jquants_earnings_date")]
    Table,
    Code,
    FqName,
    PubDate,
    SchDate,
    Fye,
    CoName,
    CoNameEn,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // 予定日の変更は新しい公表日の行として返るため、(code, fq_name, pub_date) を
        // 複合主キーとして公表日ごとの行をすべて残す (上書きしない)。
        manager
            .create_table(
                Table::create()
                    .table(JQuantsEarningsDate::Table)
                    .col(
                        ColumnDef::new(JQuantsEarningsDate::Code)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(JQuantsEarningsDate::FqName)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(JQuantsEarningsDate::PubDate)
                            .date()
                            .not_null(),
                    )
                    // 決算発表予定日が未定の場合は空文字で返るため、NULL として保存する。
                    .col(ColumnDef::new(JQuantsEarningsDate::SchDate).date())
                    .col(ColumnDef::new(JQuantsEarningsDate::Fye).string().not_null())
                    .col(
                        ColumnDef::new(JQuantsEarningsDate::CoName)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(JQuantsEarningsDate::CoNameEn)
                            .string()
                            .not_null(),
                    )
                    .primary_key(
                        Index::create()
                            .col(JQuantsEarningsDate::Code)
                            .col(JQuantsEarningsDate::FqName)
                            .col(JQuantsEarningsDate::PubDate),
                    )
                    .to_owned(),
            )
            .await?;

        // 取り込み済みかどうかの判定 (MAX(pub_date)) に使う
        manager
            .create_index(
                Index::create()
                    .name("idx_jquants_earnings_date_pub_date")
                    .table(JQuantsEarningsDate::Table)
                    .col(JQuantsEarningsDate::PubDate)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(JQuantsEarningsDate::Table).to_owned())
            .await
    }
}
