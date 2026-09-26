use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20260926_063200_remove_hypothesis_tables"
    }
}

const HYPOTHESIS_STATUSES: [&str; 4] = ["unverified", "supported", "refuted", "obsolete"];
const PROPOSAL_STATUSES: [&str; 3] = ["pending", "approved", "rejected"];

#[derive(DeriveIden)]
#[allow(clippy::enum_variant_names)]
enum Hypothesis {
    Table,
    HypothesisId,
    StrategyId,
    Title,
    Body,
    Status,
    RelatedNoteIds,
    RelatedInterestIds,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
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

#[derive(DeriveIden)]
enum Strategy {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum Note {
    Table,
    Id,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
                "DO $$
                BEGIN
                    IF EXISTS (SELECT 1 FROM hypothesis LIMIT 1)
                    OR EXISTS (SELECT 1 FROM hypothesis_proposal LIMIT 1)
                    OR EXISTS (SELECT 1 FROM note_hypothesis LIMIT 1) THEN
                        RAISE EXCEPTION 'cannot drop hypothesis tables while any table contains data';
                    END IF;
                END;
                $$;",
            )
            .await?;

        manager
            .drop_table(Table::drop().table(HypothesisProposal::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(NoteHypothesis::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Hypothesis::Table).to_owned())
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Hypothesis::Table)
                    .col(
                        ColumnDef::new(Hypothesis::HypothesisId)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Hypothesis::StrategyId).uuid())
                    .col(ColumnDef::new(Hypothesis::Title).text().not_null())
                    .col(ColumnDef::new(Hypothesis::Body).text().not_null())
                    .col(
                        ColumnDef::new(Hypothesis::Status)
                            .text()
                            .not_null()
                            .default("unverified"),
                    )
                    .col(
                        ColumnDef::new(Hypothesis::RelatedNoteIds)
                            .array(ColumnType::Uuid)
                            .not_null()
                            .default(Expr::cust("'{}'::uuid[]")),
                    )
                    .col(
                        ColumnDef::new(Hypothesis::RelatedInterestIds)
                            .array(ColumnType::Uuid)
                            .not_null()
                            .default(Expr::cust("'{}'::uuid[]")),
                    )
                    .col(
                        ColumnDef::new(Hypothesis::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(Hypothesis::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(Hypothesis::Table, Hypothesis::StrategyId)
                            .to(Strategy::Table, Strategy::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .check(Expr::col(Hypothesis::Status).is_in(HYPOTHESIS_STATUSES))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_hypothesis_strategy_updated")
                    .table(Hypothesis::Table)
                    .col(Hypothesis::StrategyId)
                    .col((Hypothesis::UpdatedAt, IndexOrder::Desc))
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
}
