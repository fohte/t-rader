use sea_orm_migration::{prelude::*, sea_query::extension::postgres::Type};

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20260911_153846_add_strategy_task_step"
    }
}

#[derive(DeriveIden)]
enum StrategyTask {
    Table,
    TaskId,
    Steps,
}

#[derive(DeriveIden)]
enum StrategyTaskStep {
    Table,
    ExecutionStepId,
    Seq,
    TaskId,
    PhaseKey,
    Label,
    Model,
    Status,
    Item,
    ItemLabel,
    Output,
    StartedAt,
    FinishedAt,
    TraceId,
    SpanId,
    Error,
}

#[derive(DeriveIden)]
struct StrategyTaskStepStatus;

#[derive(DeriveIden)]
enum StrategyTaskStepStatusVariant {
    Running,
    Completed,
    Failed,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_type(
                Type::create()
                    .as_enum(StrategyTaskStepStatus)
                    .values([
                        StrategyTaskStepStatusVariant::Running,
                        StrategyTaskStepStatusVariant::Completed,
                        StrategyTaskStepStatusVariant::Failed,
                    ])
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(StrategyTaskStep::Table)
                    .col(
                        ColumnDef::new(StrategyTaskStep::ExecutionStepId)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(StrategyTaskStep::TaskId).uuid().not_null())
                    .col(ColumnDef::new(StrategyTaskStep::PhaseKey).text().not_null())
                    .col(ColumnDef::new(StrategyTaskStep::Label).text().not_null())
                    .col(ColumnDef::new(StrategyTaskStep::Model).text().not_null())
                    .col(
                        ColumnDef::new(StrategyTaskStep::Status)
                            .custom(StrategyTaskStepStatus)
                            .not_null(),
                    )
                    .col(ColumnDef::new(StrategyTaskStep::Item).json_binary())
                    .col(ColumnDef::new(StrategyTaskStep::ItemLabel).text())
                    .col(ColumnDef::new(StrategyTaskStep::Output).json_binary())
                    .col(
                        ColumnDef::new(StrategyTaskStep::StartedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(ColumnDef::new(StrategyTaskStep::FinishedAt).timestamp_with_time_zone())
                    .col(ColumnDef::new(StrategyTaskStep::TraceId).text().not_null())
                    .col(ColumnDef::new(StrategyTaskStep::SpanId).text().not_null())
                    .col(ColumnDef::new(StrategyTaskStep::Error).text())
                    .foreign_key(
                        ForeignKey::create()
                            .from(StrategyTaskStep::Table, StrategyTaskStep::TaskId)
                            .to(StrategyTask::Table, StrategyTask::TaskId)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // sea-query の auto_increment はテーブル作成時に主キー列にしか効かないため、
        // 挿入順を保持する連番カラムは別途 raw SQL で BIGSERIAL として追加する。
        manager
            .get_connection()
            .execute_unprepared("ALTER TABLE strategy_task_step ADD COLUMN seq BIGSERIAL")
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("strategy_task_step_task_seq_idx")
                    .table(StrategyTaskStep::Table)
                    .col(StrategyTaskStep::TaskId)
                    .col(StrategyTaskStep::Seq)
                    .to_owned(),
            )
            .await?;

        // steps 列を drop する前に、既存行の jsonb 配列を strategy_task_step へ展開する。
        // 旧形式の要素には execution_step_id が無いため gen_random_uuid() で採番する
        // (t-rader-agent 側の実行と紐づかない、移行専用の識別子になる)。配列内の順序を
        // ordinality で保持し、seq (BIGSERIAL) に反映させる。
        manager
            .get_connection()
            .execute_unprepared(
                "INSERT INTO strategy_task_step (
                    execution_step_id, task_id, phase_key, label, model, status,
                    item, item_label, output, started_at, finished_at, trace_id, span_id, error
                )
                SELECT
                    gen_random_uuid(),
                    t.task_id,
                    step ->> 'phase_key',
                    step ->> 'label',
                    step ->> 'model',
                    (step ->> 'status')::strategy_task_step_status,
                    step -> 'item',
                    step ->> 'item_label',
                    step -> 'output',
                    (step ->> 'started_at')::timestamptz,
                    (step ->> 'finished_at')::timestamptz,
                    step ->> 'trace_id',
                    step ->> 'span_id',
                    step ->> 'error'
                FROM strategy_task t
                CROSS JOIN LATERAL jsonb_array_elements(t.steps) WITH ORDINALITY AS elems(step, ord)
                ORDER BY t.task_id, ord",
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(StrategyTask::Table)
                    .drop_column(StrategyTask::Steps)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
                "ALTER TABLE strategy_task ADD COLUMN steps jsonb NOT NULL DEFAULT '[]'::jsonb",
            )
            .await?;
        manager
            .get_connection()
            .execute_unprepared("ALTER TABLE strategy_task ALTER COLUMN steps DROP DEFAULT")
            .await?;

        manager
            .drop_table(Table::drop().table(StrategyTaskStep::Table).to_owned())
            .await?;

        manager
            .drop_type(Type::drop().name(StrategyTaskStepStatus).to_owned())
            .await?;

        Ok(())
    }
}
