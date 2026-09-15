use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20260915_120353_add_annotation_execution_tracking"
    }
}

#[derive(DeriveIden)]
enum Annotation {
    Table,
    StrategyId,
    ExecutionStepId,
    ExecutionTaskId,
}

#[derive(DeriveIden)]
enum Idx {
    #[sea_orm(iden = "idx_annotation_strategy_id_execution_step_id")]
    AnnotationStrategyIdExecutionStepId,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Annotation::Table)
                    .add_column_if_not_exists(ColumnDef::new(Annotation::ExecutionStepId).uuid())
                    .add_column_if_not_exists(ColumnDef::new(Annotation::ExecutionTaskId).text())
                    .to_owned(),
            )
            .await?;

        // resume 時に「同じステップの前の試行が作った未レビューのアノテーションを置き換える」
        // 検索で使う (strategy_id, execution_step_id) 絞り込みを支える。
        manager
            .create_index(
                Index::create()
                    .name(Idx::AnnotationStrategyIdExecutionStepId.to_string())
                    .table(Annotation::Table)
                    .col(Annotation::StrategyId)
                    .col(Annotation::ExecutionStepId)
                    .to_owned(),
            )
            .await?;

        // 戦略を持たないアノテーションは戦略タスク実行 (execution) に紐づかないため、
        // execution_step_id / execution_task_id を禁止する (note と同じ理由・パターン)。
        manager
            .get_connection()
            .execute_unprepared(
                "ALTER TABLE annotation ADD CONSTRAINT annotation_strategy_id_execution_check \
                 CHECK (strategy_id IS NOT NULL OR (execution_step_id IS NULL AND execution_task_id IS NULL))",
            )
            .await
            .map(|_| ())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
                "ALTER TABLE annotation DROP CONSTRAINT annotation_strategy_id_execution_check",
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name(Idx::AnnotationStrategyIdExecutionStepId.to_string())
                    .table(Annotation::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Annotation::Table)
                    .drop_column(Annotation::ExecutionStepId)
                    .drop_column(Annotation::ExecutionTaskId)
                    .to_owned(),
            )
            .await
    }
}
