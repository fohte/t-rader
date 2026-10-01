use sea_orm_migration::prelude::*;

const STATUSES: [&str; 3] = ["running", "succeeded", "failed"];

#[derive(DeriveIden)]
enum IngestRun {
    Table,
    Id,
    Job,
    StartedAt,
    FinishedAt,
    Status,
    Stats,
    Error,
}

#[derive(DeriveIden)]
enum Idx {
    #[sea_orm(iden = "idx_ingest_run_job_started_at")]
    JobStartedAt,
}

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20260930_162756_add_ingest_run"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(IngestRun::Table)
                    .col(
                        ColumnDef::new(IngestRun::Id)
                            .uuid()
                            .not_null()
                            .primary_key()
                            .default(Expr::cust("gen_random_uuid()")),
                    )
                    .col(ColumnDef::new(IngestRun::Job).text().not_null())
                    .col(
                        ColumnDef::new(IngestRun::StartedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(ColumnDef::new(IngestRun::FinishedAt).timestamp_with_time_zone())
                    .col(
                        ColumnDef::new(IngestRun::Status)
                            .text()
                            .not_null()
                            .default("running")
                            .check(Expr::col(IngestRun::Status).is_in(STATUSES)),
                    )
                    .col(ColumnDef::new(IngestRun::Stats).json_binary())
                    .col(ColumnDef::new(IngestRun::Error).text())
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name(Idx::JobStartedAt.to_string())
                    .table(IngestRun::Table)
                    .col(IngestRun::Job)
                    .col((IngestRun::StartedAt, IndexOrder::Desc))
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(IngestRun::Table).to_owned())
            .await
    }
}
