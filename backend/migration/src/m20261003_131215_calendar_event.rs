use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20261003_131215_calendar_event"
    }
}

#[derive(DeriveIden)]
enum CalendarEvent {
    Table,
    Id,
    Source,
    ExternalId,
    Category,
    Country,
    Title,
    StockId,
    FiscalPeriod,
    EventDate,
    EventAt,
    TimeOfDay,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum EarningsScheduleIngestedDate {
    Table,
    Date,
}

#[derive(DeriveIden)]
enum JQuantsEarningsDate {
    #[sea_orm(iden = "jquants_earnings_date")]
    Table,
    Code,
    FqName,
    PubDate,
    SchDate,
    Fye,
    CoName,
    CoNameEn,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(CalendarEvent::Table)
                    .col(
                        ColumnDef::new(CalendarEvent::Id)
                            .uuid()
                            .not_null()
                            .default(Expr::cust("gen_random_uuid()")),
                    )
                    .col(ColumnDef::new(CalendarEvent::Source).text().not_null())
                    .col(
                        ColumnDef::new(CalendarEvent::ExternalId)
                            .text()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(CalendarEvent::Category)
                            .text()
                            .not_null()
                            .extra("CHECK (category IN ('indicator', 'central_bank', 'earnings'))"),
                    )
                    .col(ColumnDef::new(CalendarEvent::Country).text().not_null())
                    .col(ColumnDef::new(CalendarEvent::Title).text().not_null())
                    .col(ColumnDef::new(CalendarEvent::StockId).text())
                    .col(ColumnDef::new(CalendarEvent::FiscalPeriod).text())
                    .col(ColumnDef::new(CalendarEvent::EventDate).date().not_null())
                    .col(ColumnDef::new(CalendarEvent::EventAt).timestamp_with_time_zone())
                    .col(
                        ColumnDef::new(CalendarEvent::TimeOfDay)
                            .text()
                            .extra("CHECK (time_of_day IN ('pre_market', 'post_market'))"),
                    )
                    .col(
                        ColumnDef::new(CalendarEvent::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(CalendarEvent::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .primary_key(Index::create().col(CalendarEvent::Id).primary())
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("uq_calendar_event_source_external_id")
                    .table(CalendarEvent::Table)
                    .col(CalendarEvent::Source)
                    .col(CalendarEvent::ExternalId)
                    .unique()
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_calendar_event_event_date")
                    .table(CalendarEvent::Table)
                    .col(CalendarEvent::EventDate)
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_calendar_event_source_event_date")
                    .table(CalendarEvent::Table)
                    .col(CalendarEvent::Source)
                    .col(CalendarEvent::EventDate)
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_calendar_event_stock_id")
                    .table(CalendarEvent::Table)
                    .col(CalendarEvent::StockId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(EarningsScheduleIngestedDate::Table)
                    .col(
                        ColumnDef::new(EarningsScheduleIngestedDate::Date)
                            .date()
                            .not_null(),
                    )
                    .primary_key(
                        Index::create()
                            .col(EarningsScheduleIngestedDate::Date)
                            .primary(),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .get_connection()
            .execute_unprepared(
                "WITH latest_schedule AS (\
                    SELECT DISTINCT ON (code, fq_name) code, fq_name, pub_date, sch_date, co_name \
                    FROM jquants_earnings_date \
                    ORDER BY code, fq_name, pub_date DESC\
                ) \
                INSERT INTO calendar_event (\
                    source, external_id, category, country, title, stock_id, fiscal_period, event_date\
                ) \
                SELECT 'jquants', code || ':' || fq_name, 'earnings', 'JP', co_name, code, fq_name, sch_date \
                FROM latest_schedule \
                WHERE sch_date IS NOT NULL",
            )
            .await?;
        manager
            .get_connection()
            .execute_unprepared(
                "INSERT INTO earnings_schedule_ingested_date (date) \
                 SELECT DISTINCT pub_date FROM jquants_earnings_date \
                 ON CONFLICT (date) DO NOTHING",
            )
            .await?;
        manager
            .drop_table(
                Table::drop()
                    .table(JQuantsEarningsDate::Table)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(EarningsScheduleIngestedDate::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(Table::drop().table(CalendarEvent::Table).to_owned())
            .await?;
        manager
            .create_table(
                Table::create()
                    .table(JQuantsEarningsDate::Table)
                    .col(ColumnDef::new(JQuantsEarningsDate::Code).string().not_null())
                    .col(
                        ColumnDef::new(JQuantsEarningsDate::FqName)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(JQuantsEarningsDate::PubDate)
                            .date()
                            .not_null(),
                    )
                    .col(ColumnDef::new(JQuantsEarningsDate::SchDate).date())
                    .col(ColumnDef::new(JQuantsEarningsDate::Fye).string().not_null())
                    .col(
                        ColumnDef::new(JQuantsEarningsDate::CoName)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(JQuantsEarningsDate::CoNameEn)
                            .string()
                            .not_null(),
                    )
                    .primary_key(
                        Index::create()
                            .col(JQuantsEarningsDate::Code)
                            .col(JQuantsEarningsDate::FqName)
                            .col(JQuantsEarningsDate::PubDate)
                            .primary(),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_jquants_earnings_date_pub_date")
                    .table(JQuantsEarningsDate::Table)
                    .col(JQuantsEarningsDate::PubDate)
                    .to_owned(),
            )
            .await
    }
}
