use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20261003_163435_drop_note_annotation_strategy_id"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_index(
                Index::create()
                    .name("idx_note_execution_id")
                    .table(Note::Table)
                    .col(Note::ExecutionId)
                    .unique()
                    .and_where(Expr::col(Note::ExecutionId).is_not_null())
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_annotation_execution_step_id")
                    .table(Annotation::Table)
                    .col(Annotation::ExecutionStepId)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx_note_strategy_id_execution_id")
                    .table(Note::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_index(
                Index::drop()
                    .name("idx_note_strategy_id")
                    .table(Note::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_foreign_key(
                ForeignKey::drop()
                    .name("note_strategy_id_fkey")
                    .table(Note::Table)
                    .to_owned(),
            )
            .await?;
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
                    .drop_column(Note::StrategyId)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx_annotation_strategy_id_execution_step_id")
                    .table(Annotation::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_index(
                Index::drop()
                    .name("idx_annotation_strategy_id")
                    .table(Annotation::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_foreign_key(
                ForeignKey::drop()
                    .name("annotation_strategy_id_fkey")
                    .table(Annotation::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .get_connection()
            .execute_unprepared(
                "ALTER TABLE annotation DROP CONSTRAINT annotation_strategy_id_execution_check",
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(Annotation::Table)
                    .drop_column(Annotation::StrategyId)
                    .to_owned(),
            )
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // 削除した strategy_id の値は復元できないため、strategy_id を前提とする CHECK 制約は戻せない。
        manager
            .alter_table(
                Table::alter()
                    .table(Note::Table)
                    .add_column(ColumnDef::new(Note::StrategyId).uuid().null())
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_note_strategy_id")
                    .table(Note::Table)
                    .col(Note::StrategyId)
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_note_strategy_id_execution_id")
                    .table(Note::Table)
                    .col(Note::StrategyId)
                    .col(Note::ExecutionId)
                    .unique()
                    .and_where(Expr::col(Note::ExecutionId).is_not_null())
                    .to_owned(),
            )
            .await?;
        manager
            .create_foreign_key(
                ForeignKey::create()
                    .name("note_strategy_id_fkey")
                    .from(Note::Table, Note::StrategyId)
                    .to(Strategy::Table, Strategy::Id)
                    .on_delete(ForeignKeyAction::Cascade)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Annotation::Table)
                    .add_column(ColumnDef::new(Annotation::StrategyId).uuid().null())
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_annotation_strategy_id")
                    .table(Annotation::Table)
                    .col(Annotation::StrategyId)
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_annotation_strategy_id_execution_step_id")
                    .table(Annotation::Table)
                    .col(Annotation::StrategyId)
                    .col(Annotation::ExecutionStepId)
                    .to_owned(),
            )
            .await?;
        manager
            .create_foreign_key(
                ForeignKey::create()
                    .name("annotation_strategy_id_fkey")
                    .from(Annotation::Table, Annotation::StrategyId)
                    .to(Strategy::Table, Strategy::Id)
                    .on_delete(ForeignKeyAction::Cascade)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx_note_execution_id")
                    .table(Note::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_index(
                Index::drop()
                    .name("idx_annotation_execution_step_id")
                    .table(Annotation::Table)
                    .to_owned(),
            )
            .await?;
        Ok(())
    }
}

#[derive(DeriveIden)]
enum Note {
    Table,
    StrategyId,
    ExecutionId,
}

#[derive(DeriveIden)]
enum Annotation {
    Table,
    StrategyId,
    ExecutionStepId,
}

#[derive(DeriveIden)]
enum Strategy {
    Table,
    Id,
}
