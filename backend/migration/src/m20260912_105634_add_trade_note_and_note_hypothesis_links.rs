use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20260912_105634_add_trade_note_and_note_hypothesis_links"
    }
}

#[derive(DeriveIden)]
enum Trade {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum Note {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum Hypothesis {
    Table,
    HypothesisId,
}

#[derive(DeriveIden)]
enum TradeNote {
    Table,
    TradeId,
    NoteId,
    CreatedAt,
}

#[derive(DeriveIden)]
enum NoteHypothesis {
    Table,
    NoteId,
    HypothesisId,
    HypothesisTitle,
    HypothesisBody,
    HypothesisStatus,
    CreatedAt,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(TradeNote::Table)
                    .col(ColumnDef::new(TradeNote::TradeId).uuid().not_null())
                    .col(ColumnDef::new(TradeNote::NoteId).uuid().not_null())
                    .col(
                        ColumnDef::new(TradeNote::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .primary_key(
                        Index::create()
                            .col(TradeNote::TradeId)
                            .col(TradeNote::NoteId),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(TradeNote::Table, TradeNote::TradeId)
                            .to(Trade::Table, Trade::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(TradeNote::Table, TradeNote::NoteId)
                            .to(Note::Table, Note::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(NoteHypothesis::Table)
                    .col(ColumnDef::new(NoteHypothesis::NoteId).uuid().not_null())
                    .col(
                        ColumnDef::new(NoteHypothesis::HypothesisId)
                            .uuid()
                            .not_null(),
                    )
                    // hypothesis_title/body/status はリンク作成時点の仮説内容の snapshot。
                    // 後から仮説本体が編集されても、この行は書き換えない (過去の判断根拠を固定する)。
                    .col(
                        ColumnDef::new(NoteHypothesis::HypothesisTitle)
                            .text()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(NoteHypothesis::HypothesisBody)
                            .text()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(NoteHypothesis::HypothesisStatus)
                            .text()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(NoteHypothesis::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .primary_key(
                        Index::create()
                            .col(NoteHypothesis::NoteId)
                            .col(NoteHypothesis::HypothesisId),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(NoteHypothesis::Table, NoteHypothesis::NoteId)
                            .to(Note::Table, Note::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(NoteHypothesis::Table, NoteHypothesis::HypothesisId)
                            .to(Hypothesis::Table, Hypothesis::HypothesisId)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(NoteHypothesis::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(TradeNote::Table).to_owned())
            .await?;
        Ok(())
    }
}
