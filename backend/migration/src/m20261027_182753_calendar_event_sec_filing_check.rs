use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(DeriveIden)]
enum CalendarEvent {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum CalendarEventSecFilingCheck {
    Table,
    CalendarEventId,
    CheckedAt,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(CalendarEventSecFilingCheck::Table)
                    .col(
                        ColumnDef::new(CalendarEventSecFilingCheck::CalendarEventId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(CalendarEventSecFilingCheck::CheckedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .primary_key(
                        Index::create()
                            .col(CalendarEventSecFilingCheck::CalendarEventId)
                            .primary(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_calendar_event_sec_filing_check_event")
                            .from(
                                CalendarEventSecFilingCheck::Table,
                                CalendarEventSecFilingCheck::CalendarEventId,
                            )
                            .to(CalendarEvent::Table, CalendarEvent::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(CalendarEventSecFilingCheck::Table)
                    .to_owned(),
            )
            .await
    }
}
