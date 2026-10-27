use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(DeriveIden)]
enum CalendarEvent {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum CalendarEventMacroReferenceCheck {
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
                    .table(CalendarEventMacroReferenceCheck::Table)
                    .col(
                        ColumnDef::new(CalendarEventMacroReferenceCheck::CalendarEventId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(CalendarEventMacroReferenceCheck::CheckedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .primary_key(
                        Index::create()
                            .col(CalendarEventMacroReferenceCheck::CalendarEventId)
                            .primary(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_calendar_event_macro_reference_check_event")
                            .from(
                                CalendarEventMacroReferenceCheck::Table,
                                CalendarEventMacroReferenceCheck::CalendarEventId,
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
                    .table(CalendarEventMacroReferenceCheck::Table)
                    .to_owned(),
            )
            .await
    }
}
