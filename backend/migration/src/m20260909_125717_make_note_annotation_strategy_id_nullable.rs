use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20260909_125717_make_note_annotation_strategy_id_nullable"
    }
}

#[derive(DeriveIden)]
enum Note {
    Table,
    StrategyId,
}

#[derive(DeriveIden)]
enum Annotation {
    Table,
    StrategyId,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Note::Table)
                    .modify_column(ColumnDef::new(Note::StrategyId).uuid().null())
                    .to_owned(),
            )
            .await?;

        // strategy を持たないノートは execution (戦略タスク実行) に紐づき得ないため、
        // (strategy_id, execution_id) 部分ユニークインデックスが strategy_id IS NULL の
        // 行同士を区別できなくなる問題をこの CHECK で未然に防ぐ。
        manager
            .get_connection()
            .execute_unprepared(
                "ALTER TABLE note ADD CONSTRAINT note_strategy_id_execution_id_check \
                 CHECK (strategy_id IS NOT NULL OR execution_id IS NULL)",
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Annotation::Table)
                    .modify_column(ColumnDef::new(Annotation::StrategyId).uuid().null())
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
                "ALTER TABLE note DROP CONSTRAINT note_strategy_id_execution_id_check",
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Note::Table)
                    .modify_column(ColumnDef::new(Note::StrategyId).uuid().not_null())
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Annotation::Table)
                    .modify_column(ColumnDef::new(Annotation::StrategyId).uuid().not_null())
                    .to_owned(),
            )
            .await
    }
}
