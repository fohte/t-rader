use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20260912_130617_add_hypothesis_proposal"
    }
}

const HYPOTHESIS_STATUSES: [&str; 4] = ["unverified", "supported", "refuted", "obsolete"];
const PROPOSAL_STATUSES: [&str; 3] = ["pending", "approved", "rejected"];

#[derive(DeriveIden)]
enum Hypothesis {
    Table,
    HypothesisId,
}

#[derive(DeriveIden)]
#[allow(clippy::enum_variant_names)]
enum HypothesisProposal {
    Table,
    Id,
    HypothesisId,
    ProposedTitle,
    ProposedBody,
    ProposedStatus,
    Rationale,
    Status,
    ReviewNote,
    CreatedAt,
    ReviewedAt,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(HypothesisProposal::Table)
                    .col(
                        ColumnDef::new(HypothesisProposal::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(HypothesisProposal::HypothesisId)
                            .uuid()
                            .not_null(),
                    )
                    .col(ColumnDef::new(HypothesisProposal::ProposedTitle).text())
                    .col(ColumnDef::new(HypothesisProposal::ProposedBody).text())
                    .col(ColumnDef::new(HypothesisProposal::ProposedStatus).text())
                    .col(
                        ColumnDef::new(HypothesisProposal::Rationale)
                            .text()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(HypothesisProposal::Status)
                            .text()
                            .not_null()
                            .default("pending"),
                    )
                    .col(ColumnDef::new(HypothesisProposal::ReviewNote).text())
                    .col(
                        ColumnDef::new(HypothesisProposal::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(ColumnDef::new(HypothesisProposal::ReviewedAt).timestamp_with_time_zone())
                    .foreign_key(
                        ForeignKey::create()
                            .from(HypothesisProposal::Table, HypothesisProposal::HypothesisId)
                            .to(Hypothesis::Table, Hypothesis::HypothesisId)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .check(Expr::col(HypothesisProposal::ProposedStatus).is_null().or(
                        Expr::col(HypothesisProposal::ProposedStatus).is_in(HYPOTHESIS_STATUSES),
                    ))
                    .check(Expr::col(HypothesisProposal::Status).is_in(PROPOSAL_STATUSES))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_hypothesis_proposal_hypothesis_created")
                    .table(HypothesisProposal::Table)
                    .col(HypothesisProposal::HypothesisId)
                    .col((HypothesisProposal::CreatedAt, IndexOrder::Desc))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_hypothesis_proposal_status_created")
                    .table(HypothesisProposal::Table)
                    .col(HypothesisProposal::Status)
                    .col((HypothesisProposal::CreatedAt, IndexOrder::Desc))
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(HypothesisProposal::Table).to_owned())
            .await?;
        Ok(())
    }
}
