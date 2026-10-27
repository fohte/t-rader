use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20261025_175545_calendar_event_reference"
    }
}

#[derive(DeriveIden)]
enum CalendarEvent {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum CalendarEventReference {
    Table,
    Id,
    CalendarEventId,
    Kind,
    Source,
    ExternalId,
    Title,
    Url,
    TargetPeriodStart,
    TargetPeriodEnd,
    TargetPeriodType,
    PublicationEvidence,
    PublicationEvidenceUrl,
    CheckedAt,
    CreatedAt,
    UpdatedAt,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(CalendarEventReference::Table)
                    .col(
                        ColumnDef::new(CalendarEventReference::Id)
                            .uuid()
                            .not_null()
                            .default(Expr::cust("gen_random_uuid()")),
                    )
                    .col(
                        ColumnDef::new(CalendarEventReference::CalendarEventId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(CalendarEventReference::Kind)
                            .text()
                            .not_null()
                            .extra("CHECK (kind IN ('earnings_summary', 'individual_document', 'official_listing'))"),
                    )
                    .col(ColumnDef::new(CalendarEventReference::Source).text().not_null())
                    .col(
                        ColumnDef::new(CalendarEventReference::ExternalId)
                            .text()
                            .not_null(),
                    )
                    .col(ColumnDef::new(CalendarEventReference::Title).text().not_null())
                    .col(ColumnDef::new(CalendarEventReference::Url).text())
                    .col(ColumnDef::new(CalendarEventReference::TargetPeriodStart).date())
                    .col(ColumnDef::new(CalendarEventReference::TargetPeriodEnd).date())
                    .col(ColumnDef::new(CalendarEventReference::TargetPeriodType).text())
                    .col(ColumnDef::new(CalendarEventReference::PublicationEvidence).text())
                    .col(ColumnDef::new(CalendarEventReference::PublicationEvidenceUrl).text())
                    .col(
                        ColumnDef::new(CalendarEventReference::CheckedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(CalendarEventReference::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(CalendarEventReference::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .primary_key(Index::create().col(CalendarEventReference::Id).primary())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_calendar_event_reference_event")
                            .from(CalendarEventReference::Table, CalendarEventReference::CalendarEventId)
                            .to(CalendarEvent::Table, CalendarEvent::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .get_connection()
            .execute_unprepared(
                "ALTER TABLE calendar_event_reference \
                 ADD CONSTRAINT calendar_event_reference_period_range_check \
                 CHECK (target_period_start IS NULL OR target_period_end IS NULL OR target_period_start <= target_period_end); \
                 ALTER TABLE calendar_event_reference \
                 ADD CONSTRAINT calendar_event_reference_document_evidence_check \
                 CHECK (kind <> 'individual_document' OR publication_evidence IS NOT NULL)",
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("uq_calendar_event_reference_event_source_external_id")
                    .table(CalendarEventReference::Table)
                    .col(CalendarEventReference::CalendarEventId)
                    .col(CalendarEventReference::Source)
                    .col(CalendarEventReference::ExternalId)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_calendar_event_reference_event_kind")
                    .table(CalendarEventReference::Table)
                    .col(CalendarEventReference::CalendarEventId)
                    .col(CalendarEventReference::Kind)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(CalendarEventReference::Table).to_owned())
            .await
    }
}
