use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(DeriveIden)]
enum Note {
    Table,
    StrategyId,
    ExecutionId,
}

#[derive(DeriveIden)]
enum Idx {
    #[sea_orm(iden = "idx_note_strategy_id_execution_id")]
    NoteStrategyIdExecutionId,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Note::Table)
                    .add_column_if_not_exists(ColumnDef::new(Note::ExecutionId).text().null())
                    .to_owned(),
            )
            .await?;

        // 同一実行 (execution_id) 内での write_note リトライを同一ノートへ収束させるための
        // 部分ユニークインデックス。execution_id が NULL の行 (非対応クライアント) は対象外。
        manager
            .create_index(
                Index::create()
                    .name(Idx::NoteStrategyIdExecutionId.to_string())
                    .table(Note::Table)
                    .col(Note::StrategyId)
                    .col(Note::ExecutionId)
                    .unique()
                    .and_where(Expr::col(Note::ExecutionId).is_not_null())
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .name(Idx::NoteStrategyIdExecutionId.to_string())
                    .table(Note::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Note::Table)
                    .drop_column(Note::ExecutionId)
                    .to_owned(),
            )
            .await
    }
}
