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
            .get_connection()
            .execute_unprepared(indoc::indoc! {"
                DO $$
                BEGIN
                    IF EXISTS (
                        SELECT 1
                        FROM jquants_earnings_date
                        WHERE sch_date IS NOT NULL
                          AND (
                              fq_name NOT IN ('1Q', '2Q', '3Q', '4Q', 'FY')
                              OR fye !~ '^(0[1-9]|1[0-2])[0-9]{2}$'
                          )
                    ) THEN
                        RAISE EXCEPTION 'cannot migrate dated earnings schedules with invalid fq_name or fye';
                    END IF;
                END;
                $$;
            "})
            .await?;

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

        // 未定への訂正より先に有日程の最新行を選び、古い予定日が復活しないようにする。
        manager
            .get_connection()
            .execute_unprepared(indoc::indoc! {"
                WITH dated_schedules AS (
                    SELECT
                        schedule.code,
                        schedule.fq_name,
                        schedule.pub_date,
                        schedule.sch_date,
                        schedule.co_name,
                        CASE
                            WHEN quarter_end_in_schedule_year.quarter_end < schedule.sch_date
                                THEN quarter_end_in_schedule_year.quarter_end
                            ELSE (quarter_end_in_schedule_year.quarter_end - INTERVAL '1 year')::date
                        END AS quarter_end
                    FROM jquants_earnings_date AS schedule
                    CROSS JOIN LATERAL (
                        SELECT (
                            make_date(
                                EXTRACT(YEAR FROM schedule.sch_date)::integer,
                                (
                                    (
                                        substring(schedule.fye FROM 1 FOR 2)::integer
                                        - (
                                            4 - CASE schedule.fq_name
                                                WHEN 'FY' THEN 4
                                                ELSE substring(schedule.fq_name FROM 1 FOR 1)::integer
                                            END
                                        ) * 3
                                        - 1 + 12
                                    ) % 12
                                ) + 1,
                                1
                            ) + INTERVAL '1 month' - INTERVAL '1 day'
                        )::date AS quarter_end
                    ) AS quarter_end_in_schedule_year
                    WHERE schedule.sch_date IS NOT NULL
                      AND schedule.fq_name IN ('1Q', '2Q', '3Q', '4Q', 'FY')
                      AND schedule.fye ~ '^(0[1-9]|1[0-2])[0-9]{2}$'
                ), latest_dated_candidates AS (
                    SELECT DISTINCT ON (code, quarter_end)
                        code, fq_name, pub_date, sch_date, co_name, quarter_end
                    FROM dated_schedules
                    ORDER BY code, quarter_end, pub_date DESC, fq_name
                )
                INSERT INTO calendar_event (
                    source, external_id, category, country, title, stock_id, fiscal_period, event_date
                )
                SELECT
                    'jquants',
                    code || ':' || fq_name || ':' || to_char(quarter_end, 'YYYY-MM-DD'),
                    'earnings',
                    'JP',
                    co_name,
                    code,
                    to_char(quarter_end, 'YYYY-MM-DD'),
                    sch_date
                FROM latest_dated_candidates AS candidate
                WHERE NOT EXISTS (
                    SELECT 1
                    FROM jquants_earnings_date AS unknown_schedule
                    WHERE unknown_schedule.code = candidate.code
                      AND unknown_schedule.fq_name = candidate.fq_name
                      AND unknown_schedule.pub_date > candidate.pub_date
                      AND unknown_schedule.sch_date IS NULL
                      AND candidate.sch_date >= unknown_schedule.pub_date
                )
            "})
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

#[cfg(test)]
mod tests {
    #![allow(
        clippy::expect_used,
        reason = "テスト準備の失敗時に詳細を表示するため"
    )]

    use sea_orm::sea_query::{Alias, Expr, Order, Query, SelectExprTrait};
    use sea_orm::{ConnectionTrait, Database, TransactionTrait};
    use sea_orm_migration::{MigrationTrait, SchemaManager};

    use super::Migration;

    #[tokio::test]
    async fn migration_moves_fiscal_year_events_and_preserves_other_quarters() {
        let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
        let database = Database::connect(database_url)
            .await
            .expect("connect to test database");
        let transaction = database.begin().await.expect("begin transaction");
        let schema = format!("calendar_event_fy_test_{}", std::process::id());
        transaction
            .execute_unprepared(&format!("CREATE SCHEMA {schema}"))
            .await
            .expect("create isolated schema");
        transaction
            .execute_unprepared(&format!("SET LOCAL search_path TO {schema}"))
            .await
            .expect("set isolated schema");
        transaction
            .execute_unprepared(
                "CREATE TABLE jquants_earnings_date (
                    code text NOT NULL,
                    fq_name text NOT NULL,
                    pub_date date NOT NULL,
                    sch_date date,
                    fye text NOT NULL,
                    co_name text NOT NULL,
                    co_name_en text NOT NULL
                )",
            )
            .await
            .expect("create source table");
        transaction
            .execute_unprepared(
                "INSERT INTO jquants_earnings_date
                    (code, fq_name, pub_date, sch_date, fye, co_name, co_name_en)
                 VALUES
                    ('ZZ999', 'FY', '2042-07-06', '2043-03-05', '0297', '架空社', 'Fictional Co'),
                    ('AA123', 'FY', '2042-07-06', '2042-10-01', '1231', '別の架空社', 'Another Fictional Co'),
                    ('AA123', 'FY', '2042-08-15', NULL, '1231', '別の架空社', 'Another Fictional Co'),
                    ('ZZ100', '1Q', '2042-07-06', '2043-01-05', '1231', '四半期社', 'Quarter Co'),
                    ('ZZ100', '2Q', '2042-07-06', '2043-01-05', '1231', '四半期社', 'Quarter Co'),
                    ('ZZ100', '3Q', '2042-07-06', '2043-01-05', '1231', '四半期社', 'Quarter Co')",
            )
            .await
            .expect("insert source schedules");

        Migration
            .up(&SchemaManager::new(&transaction))
            .await
            .expect("run calendar event migration");

        let rows = transaction
            .query_all(
                &Query::select()
                    .column(Alias::new("external_id"))
                    .column(Alias::new("fiscal_period"))
                    .expr(Expr::cust("event_date::text").alias("event_date"))
                    .from(Alias::new("calendar_event"))
                    .order_by(Alias::new("external_id"), Order::Asc)
                    .to_owned(),
            )
            .await
            .expect("read migrated events")
            .into_iter()
            .map(|row| {
                (
                    row.try_get::<String>("", "external_id")
                        .expect("read external id"),
                    row.try_get::<String>("", "fiscal_period")
                        .expect("read fiscal period"),
                    row.try_get::<String>("", "event_date")
                        .expect("read event date"),
                )
            })
            .collect::<Vec<_>>();

        assert_eq!(
            rows,
            vec![
                (
                    "ZZ100:1Q:2042-03-31".into(),
                    "2042-03-31".into(),
                    "2043-01-05".into(),
                ),
                (
                    "ZZ100:2Q:2042-06-30".into(),
                    "2042-06-30".into(),
                    "2043-01-05".into(),
                ),
                (
                    "ZZ100:3Q:2042-09-30".into(),
                    "2042-09-30".into(),
                    "2043-01-05".into(),
                ),
                (
                    "ZZ999:FY:2043-02-28".into(),
                    "2043-02-28".into(),
                    "2043-03-05".into(),
                ),
            ],
        );
        transaction.rollback().await.expect("roll back test schema");
    }
}
