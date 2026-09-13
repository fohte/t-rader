use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20260913_072524_add_jquants_fin_summary"
    }
}

#[derive(DeriveIden)]
enum JQuantsFinSummary {
    // DeriveIden の既定の snake_case 変換は連続大文字 "JQ" を "j_q" と別語扱いしてしまう
    // (`j_quants_fin_summary` になる) ため、テーブル名は明示的に上書きする。
    #[sea_orm(iden = "jquants_fin_summary")]
    Table,
    Code,
    DiscNo,
    DiscDate,
    Raw,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // J-Quants が返す生レコードをそのまま raw に保存する (フィールド全量を型付きカラムに
        // 展開しない)。訂正は (code, disc_no) が同じレコードへの上書きとして扱う。
        manager
            .create_table(
                Table::create()
                    .table(JQuantsFinSummary::Table)
                    .col(ColumnDef::new(JQuantsFinSummary::Code).string().not_null())
                    .col(
                        ColumnDef::new(JQuantsFinSummary::DiscNo)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(JQuantsFinSummary::DiscDate)
                            .date()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(JQuantsFinSummary::Raw)
                            .json_binary()
                            .not_null(),
                    )
                    .primary_key(
                        Index::create()
                            .col(JQuantsFinSummary::Code)
                            .col(JQuantsFinSummary::DiscNo),
                    )
                    .to_owned(),
            )
            .await?;

        // 取り込み済みかどうかの判定 (MAX(disc_date)) に使う
        manager
            .create_index(
                Index::create()
                    .name("idx_jquants_fin_summary_disc_date")
                    .table(JQuantsFinSummary::Table)
                    .col(JQuantsFinSummary::DiscDate)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(JQuantsFinSummary::Table).to_owned())
            .await
    }
}
