use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(DeriveIden)]
enum StrategyTaskStepEvidence {
    Table,
    Id,
    ExecutionStepId,
    Source,
    SourceRef,
    ObservedAt,
    PublishedAt,
    EffectiveAt,
    Snapshot,
}

#[derive(DeriveIden)]
enum Idx {
    #[sea_orm(iden = "strategy_task_step_evidence_execution_step_id_idx")]
    StrategyTaskStepEvidenceExecutionStepId,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(StrategyTaskStepEvidence::Table)
                    .col(
                        ColumnDef::new(StrategyTaskStepEvidence::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    // strategy_task_step は非同期に反映されるため FK にしない
                    // (理由: backend/src/mcp/strategy/evidence.rs)
                    .col(
                        ColumnDef::new(StrategyTaskStepEvidence::ExecutionStepId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(StrategyTaskStepEvidence::Source)
                            .text()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(StrategyTaskStepEvidence::SourceRef)
                            .text()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(StrategyTaskStepEvidence::ObservedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(StrategyTaskStepEvidence::PublishedAt)
                            .timestamp_with_time_zone(),
                    )
                    .col(
                        ColumnDef::new(StrategyTaskStepEvidence::EffectiveAt)
                            .timestamp_with_time_zone(),
                    )
                    .col(
                        ColumnDef::new(StrategyTaskStepEvidence::Snapshot)
                            .json_binary()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name(Idx::StrategyTaskStepEvidenceExecutionStepId.to_string())
                    .table(StrategyTaskStepEvidence::Table)
                    .col(StrategyTaskStepEvidence::ExecutionStepId)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(StrategyTaskStepEvidence::Table)
                    .to_owned(),
            )
            .await
    }
}
