use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, Utc};
use core_application::calendar::repository::{
    CalendarEventRepository, CalendarEventRepositoryError,
};
use core_application::sec_filing_reference::{SecFilingReferenceTarget, SecFilingTargetRepository};
use core_application::unit_of_work::UnitOfWorkTransaction;
use core_domain::calendar_event::{
    CalendarEvent, CalendarEventCategory, CalendarEventReadModel, CalendarEventReference,
};
use sea_orm::ActiveValue::Set;
use sea_orm::sea_query::OnConflict;
use sea_orm::{ColumnTrait, EntityTrait, Iterable, QueryFilter, QueryOrder};

use crate::DatabaseHandle;
use crate::entities::calendar_event;
use crate::persistence::persistence_error;
use crate::transaction::transaction_ref as postgres_transaction_ref;

const MAX_UPSERT_ROWS_PER_STATEMENT: usize = 1_000;

mod references;

#[derive(Clone)]
pub struct PostgresCalendarEventRepository {
    db: DatabaseHandle,
}

impl PostgresCalendarEventRepository {
    pub fn new(db: DatabaseHandle) -> Self {
        Self { db }
    }
}

#[async_trait]
impl CalendarEventRepository for PostgresCalendarEventRepository {
    async fn list_events(
        &self,
        date_range: &core_application::daily_bar_source::DateRange,
    ) -> Result<Vec<CalendarEvent>, CalendarEventRepositoryError> {
        calendar_event::Entity::find()
            .filter(calendar_event::Column::EventDate.gte(date_range.from))
            .filter(calendar_event::Column::EventDate.lte(date_range.to))
            .order_by_asc(calendar_event::Column::EventDate)
            .order_by_asc(calendar_event::Column::Country)
            .order_by_asc(calendar_event::Column::Category)
            .order_by_asc(calendar_event::Column::EventAt)
            .order_by_asc(calendar_event::Column::Title)
            .order_by_asc(calendar_event::Column::Source)
            .order_by_asc(calendar_event::Column::ExternalId)
            .all(&self.db)
            .await
            .map_err(repository_error)?
            .into_iter()
            .map(to_domain)
            .collect()
    }

    async fn enrich_events(
        &self,
        events: Vec<CalendarEvent>,
    ) -> Result<Vec<CalendarEventReadModel>, CalendarEventRepositoryError> {
        references::enrich_events(self, events).await
    }

    async fn list_references_for_event(
        &self,
        event_source: &str,
        event_external_id: &str,
    ) -> Result<Vec<CalendarEventReference>, CalendarEventRepositoryError> {
        references::list_references_for_event(self, event_source, event_external_id).await
    }

    async fn upsert(
        &self,
        transaction: &UnitOfWorkTransaction,
        events: Vec<CalendarEvent>,
    ) -> Result<usize, CalendarEventRepositoryError> {
        if events.is_empty() {
            return Ok(0);
        }

        let transaction = transaction_ref(transaction)?;
        let count = events.len();
        let models = events.into_iter().map(to_active_model).collect::<Vec<_>>();
        let columns_per_row = calendar_event::Column::iter().count();
        let chunk_size = crate::repositories::max_rows_per_statement(columns_per_row)
            .min(MAX_UPSERT_ROWS_PER_STATEMENT);

        for chunk in models.chunks(chunk_size) {
            calendar_event::Entity::insert_many(chunk.to_vec())
                .on_conflict(
                    OnConflict::columns([
                        calendar_event::Column::Source,
                        calendar_event::Column::ExternalId,
                    ])
                    .update_columns([
                        calendar_event::Column::Category,
                        calendar_event::Column::Country,
                        calendar_event::Column::Title,
                        calendar_event::Column::StockId,
                        calendar_event::Column::FiscalPeriod,
                        calendar_event::Column::EventDate,
                        calendar_event::Column::EventAt,
                        calendar_event::Column::TimeOfDay,
                        calendar_event::Column::UpdatedAt,
                    ])
                    .to_owned(),
                )
                .exec_without_returning(transaction)
                .await
                .map_err(repository_error)?;
        }

        Ok(count)
    }

    async fn upsert_references(
        &self,
        transaction: &UnitOfWorkTransaction,
        references: Vec<CalendarEventReference>,
    ) -> Result<usize, CalendarEventRepositoryError> {
        references::upsert_references(transaction, references).await
    }

    async fn delete_missing_future_events(
        &self,
        transaction: &UnitOfWorkTransaction,
        source: &str,
        date_range: &core_application::daily_bar_source::DateRange,
        today: NaiveDate,
        external_ids: Vec<String>,
    ) -> Result<u64, CalendarEventRepositoryError> {
        let transaction = transaction_ref(transaction)?;
        let from = date_range.from.max(today);
        if from > date_range.to {
            return Ok(0);
        }

        let mut delete = calendar_event::Entity::delete_many()
            .filter(calendar_event::Column::Source.eq(source))
            .filter(calendar_event::Column::EventDate.gte(from))
            .filter(calendar_event::Column::EventDate.lte(date_range.to));
        if !external_ids.is_empty() {
            delete = delete.filter(calendar_event::Column::ExternalId.is_not_in(external_ids));
        }

        delete
            .exec(transaction)
            .await
            .map(|result| result.rows_affected)
            .map_err(repository_error)
    }

    async fn find_next_earnings_for_stock(
        &self,
        stock_id: &str,
        from: NaiveDate,
    ) -> Result<Option<CalendarEvent>, CalendarEventRepositoryError> {
        calendar_event::Entity::find()
            .filter(calendar_event::Column::Category.eq(CalendarEventCategory::Earnings.as_str()))
            .filter(calendar_event::Column::StockId.eq(stock_id))
            .filter(calendar_event::Column::EventDate.gte(from))
            .order_by_asc(calendar_event::Column::EventDate)
            .order_by_asc(calendar_event::Column::EventAt)
            .order_by_asc(calendar_event::Column::ExternalId)
            .one(&self.db)
            .await
            .map_err(repository_error)?
            .map(to_domain)
            .transpose()
    }
}

#[async_trait]
impl SecFilingTargetRepository for PostgresCalendarEventRepository {
    async fn list_pending_sec_filing_targets(
        &self,
        today: NaiveDate,
    ) -> Result<Vec<SecFilingReferenceTarget>, CalendarEventRepositoryError> {
        references::list_pending_sec_filing_targets(self, today).await
    }

    async fn mark_sec_filing_targets_checked(
        &self,
        targets: &[SecFilingReferenceTarget],
        checked_at: DateTime<Utc>,
    ) -> Result<(), CalendarEventRepositoryError> {
        references::mark_sec_filing_targets_checked(self, targets, checked_at).await
    }
}

fn transaction_ref(
    transaction: &UnitOfWorkTransaction,
) -> Result<&sea_orm::DatabaseTransaction, CalendarEventRepositoryError> {
    postgres_transaction_ref(transaction).ok_or(CalendarEventRepositoryError::InvalidTransaction)
}

fn to_active_model(event: CalendarEvent) -> calendar_event::ActiveModel {
    calendar_event::ActiveModel {
        source: Set(event.source),
        external_id: Set(event.external_id),
        category: Set(event.category.as_str().to_string()),
        country: Set(event.country),
        title: Set(event.title),
        stock_id: Set(event.stock_id),
        fiscal_period: Set(event.fiscal_period),
        event_date: Set(event.event_date),
        event_at: Set(event.event_at.map(|timestamp| timestamp.fixed_offset())),
        time_of_day: Set(event.time_of_day.map(|value| value.as_str().to_string())),
        updated_at: Set(Utc::now().fixed_offset()),
        ..Default::default()
    }
}

fn to_domain(row: calendar_event::Model) -> Result<CalendarEvent, CalendarEventRepositoryError> {
    let category = match row.category.as_str() {
        "indicator" => CalendarEventCategory::Indicator,
        "central_bank" => CalendarEventCategory::CentralBank,
        "earnings" => CalendarEventCategory::Earnings,
        value => {
            return Err(CalendarEventRepositoryError::Database(
                core_application::persistence::PersistenceError::Database(format!(
                    "unknown calendar event category: {value}"
                )),
            ));
        }
    };
    let time_of_day = match row.time_of_day.as_deref() {
        Some("pre_market") => Some(core_domain::calendar_event::CalendarEventTimeOfDay::PreMarket),
        Some("post_market") => {
            Some(core_domain::calendar_event::CalendarEventTimeOfDay::PostMarket)
        }
        Some(value) => {
            return Err(CalendarEventRepositoryError::Database(
                core_application::persistence::PersistenceError::Database(format!(
                    "unknown calendar event time of day: {value}"
                )),
            ));
        }
        None => None,
    };

    Ok(CalendarEvent {
        source: row.source,
        external_id: row.external_id,
        category,
        country: row.country,
        title: row.title,
        stock_id: row
            .stock_id
            .map(|stock_id| normalize_local_code(&stock_id).to_string()),
        fiscal_period: row.fiscal_period,
        event_date: row.event_date,
        event_at: row.event_at.map(|event_at| event_at.with_timezone(&Utc)),
        time_of_day,
    })
}

fn normalize_local_code(code: &str) -> &str {
    if code.len() == 5 && code.ends_with('0') {
        &code[..4]
    } else {
        code
    }
}

fn repository_error(error: sea_orm::DbErr) -> CalendarEventRepositoryError {
    CalendarEventRepositoryError::Database(persistence_error(error))
}

#[cfg(test)]
mod tests {
    use chrono::{DateTime, NaiveDate, Utc};
    use core_application::{
        calendar::repository::CalendarEventRepository, daily_bar_source::DateRange,
        financial_summary::FinancialSummaryRepository, unit_of_work::UnitOfWork,
    };
    use core_domain::calendar_event::{
        CalendarEvent, CalendarEventCategory, CalendarEventReadModel, CalendarEventReference,
        CalendarEventReferenceKind, CalendarEventTimeOfDay,
    };
    use core_domain::financial_summary::FinancialSummary;
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};

    use super::PostgresCalendarEventRepository;
    use crate::entities::calendar_event;
    use crate::{DatabaseHandle, PostgresFinancialSummaryRepository, PostgresUnitOfWork};

    fn date(year: i32, month: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(year, month, day).expect("valid date")
    }

    fn event(source: &str, external_id: &str, title: &str, event_date: NaiveDate) -> CalendarEvent {
        CalendarEvent {
            source: source.into(),
            external_id: external_id.into(),
            category: CalendarEventCategory::Earnings,
            country: "JP".into(),
            title: title.into(),
            stock_id: Some("DEMO1".into()),
            fiscal_period: Some("QX".into()),
            event_date,
            event_at: None,
            time_of_day: None,
        }
    }

    fn jp_event(stock_id: &str, period_type: &str, period_end: NaiveDate) -> CalendarEvent {
        let period_end_text = period_end.format("%Y-%m-%d").to_string();
        CalendarEvent {
            source: "jquants".into(),
            external_id: format!("{stock_id}0:{period_type}:{period_end_text}"),
            category: CalendarEventCategory::Earnings,
            country: "JP".into(),
            title: "架空銘柄".into(),
            stock_id: Some(format!("{stock_id}0")),
            fiscal_period: Some(period_end_text),
            event_date: date(2042, 8, 1),
            event_at: None,
            time_of_day: None,
        }
    }

    fn period_summary(
        code: &str,
        disclosure_no: &str,
        report_group_key: &str,
        document_type: &str,
        current_period_type: &str,
        current_period_end: NaiveDate,
    ) -> FinancialSummary {
        FinancialSummary {
            code: code.into(),
            disclosure_no: disclosure_no.into(),
            disclosure_date: date(2042, 8, 1),
            report_group_key: report_group_key.into(),
            document_type: Some(document_type.into()),
            current_period_type: Some(current_period_type.into()),
            current_period_start: None,
            current_period_end: Some(current_period_end),
            current_fiscal_year_start: None,
            current_fiscal_year_end: None,
            sales: Some(100.0),
            operating_profit: None,
            ordinary_profit: None,
            net_profit: None,
            eps: None,
            bps: None,
            total_assets: None,
            equity: None,
            equity_to_asset_ratio: None,
            roe: None,
            cash_flow_operating: None,
            cash_flow_investing: None,
            cash_flow_financing: None,
            cash_and_equivalents: None,
            dividend_annual: None,
            dividend_annual_forecast: None,
            dividend_annual_forecast_next: None,
            forecast_sales: None,
            forecast_operating_profit: None,
            forecast_ordinary_profit: None,
            forecast_net_profit: None,
            forecast_eps: None,
            next_forecast_sales: None,
            next_forecast_operating_profit: None,
            next_forecast_ordinary_profit: None,
            next_forecast_net_profit: None,
            next_forecast_eps: None,
        }
    }

    fn fields(row: calendar_event::Model) -> (String, String, String, NaiveDate) {
        (row.source, row.external_id, row.title, row.event_date)
    }

    #[backend_test_macros::database_test]
    async fn list_events_filters_inclusive_dates_and_maps_all_event_fields(db: DatabaseHandle) {
        let repository = PostgresCalendarEventRepository::new(db.clone());
        let unit_of_work = PostgresUnitOfWork::new(db);
        let transaction = unit_of_work.begin().await.expect("begin transaction");
        let mut indicator = event(
            "sample_source",
            "indicator",
            "サンプル指標",
            date(2099, 8, 10),
        );
        indicator.category = CalendarEventCategory::Indicator;
        indicator.country = "US".into();
        indicator.stock_id = None;
        indicator.fiscal_period = None;
        indicator.event_at = Some(DateTime::from_naive_utc_and_offset(
            date(2099, 8, 10)
                .and_hms_opt(13, 30, 0)
                .expect("valid time"),
            Utc,
        ));
        indicator.time_of_day = None;
        let mut central_bank = event(
            "sample_source",
            "central-bank",
            "サンプル中銀イベント",
            date(2099, 8, 11),
        );
        central_bank.category = CalendarEventCategory::CentralBank;
        central_bank.country = "EU".into();
        central_bank.stock_id = None;
        central_bank.fiscal_period = None;
        central_bank.time_of_day = None;
        let mut earnings = event("jquants", "earnings", "サンプル銘柄 A", date(2099, 8, 11));
        earnings.stock_id = Some("0001".into());
        earnings.time_of_day = Some(CalendarEventTimeOfDay::PostMarket);
        let mut stored_earnings = earnings.clone();
        stored_earnings.stock_id = Some("00010".into());
        repository
            .upsert(
                &transaction,
                vec![central_bank.clone(), stored_earnings, indicator.clone()],
            )
            .await
            .expect("insert events");
        unit_of_work
            .commit(transaction)
            .await
            .expect("commit transaction");

        let actual = repository
            .list_events(&DateRange {
                from: date(2099, 8, 10),
                to: date(2099, 8, 11),
            })
            .await
            .expect("list events");

        assert_eq!(actual, vec![indicator, central_bank, earnings]);
    }

    #[backend_test_macros::database_test]
    async fn upsert_replaces_matching_events_and_deletes_only_missing_future_events_in_range(
        db: DatabaseHandle,
    ) {
        let repository = PostgresCalendarEventRepository::new(db.clone());
        let unit_of_work = PostgresUnitOfWork::new(db.clone());
        let transaction = unit_of_work.begin().await.expect("begin transaction");
        let stored = vec![
            event("jquants", "existing", "old title", date(2099, 8, 11)),
            event("jquants", "missing", "stale event", date(2099, 8, 12)),
            event("jquants", "past", "past event", date(2099, 8, 5)),
            event("jquants", "outside", "outside range", date(2099, 9, 1)),
            event("other", "other-source", "other source", date(2099, 8, 12)),
        ];
        let inserted = repository
            .upsert(&transaction, stored)
            .await
            .expect("insert initial events");
        let updated = repository
            .upsert(
                &transaction,
                vec![event(
                    "jquants",
                    "existing",
                    "updated title",
                    date(2099, 8, 13),
                )],
            )
            .await
            .expect("upsert updated event");
        let deleted = repository
            .delete_missing_future_events(
                &transaction,
                "jquants",
                &DateRange {
                    from: date(2099, 8, 1),
                    to: date(2099, 8, 31),
                },
                date(2099, 8, 10),
                vec!["existing".into()],
            )
            .await
            .expect("delete missing future events");
        unit_of_work
            .commit(transaction)
            .await
            .expect("commit transaction");
        let rows = calendar_event::Entity::find()
            .order_by_asc(calendar_event::Column::Source)
            .order_by_asc(calendar_event::Column::ExternalId)
            .all(&db)
            .await
            .expect("list events")
            .into_iter()
            .map(fields)
            .collect::<Vec<_>>();

        assert_eq!(
            (inserted, updated, deleted, rows),
            (
                5,
                1,
                1,
                vec![
                    (
                        "jquants".into(),
                        "existing".into(),
                        "updated title".into(),
                        date(2099, 8, 13),
                    ),
                    (
                        "jquants".into(),
                        "outside".into(),
                        "outside range".into(),
                        date(2099, 9, 1),
                    ),
                    (
                        "jquants".into(),
                        "past".into(),
                        "past event".into(),
                        date(2099, 8, 5),
                    ),
                    (
                        "other".into(),
                        "other-source".into(),
                        "other source".into(),
                        date(2099, 8, 12),
                    ),
                ],
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn empty_external_ids_delete_all_missing_future_events_in_source_range(
        db: DatabaseHandle,
    ) {
        let repository = PostgresCalendarEventRepository::new(db.clone());
        let unit_of_work = PostgresUnitOfWork::new(db.clone());
        let transaction = unit_of_work.begin().await.expect("begin transaction");
        repository
            .upsert(
                &transaction,
                vec![
                    event("jquants", "future", "future event", date(2099, 8, 11)),
                    event("jquants", "past", "past event", date(2099, 8, 9)),
                    event("jquants", "outside", "outside range", date(2099, 9, 1)),
                    event("other", "other-source", "other source", date(2099, 8, 11)),
                ],
            )
            .await
            .expect("insert initial events");

        let deleted = repository
            .delete_missing_future_events(
                &transaction,
                "jquants",
                &DateRange {
                    from: date(2099, 8, 1),
                    to: date(2099, 8, 31),
                },
                date(2099, 8, 10),
                Vec::new(),
            )
            .await
            .expect("delete missing future events");
        unit_of_work
            .commit(transaction)
            .await
            .expect("commit transaction");
        let rows = calendar_event::Entity::find()
            .order_by_asc(calendar_event::Column::Source)
            .order_by_asc(calendar_event::Column::ExternalId)
            .all(&db)
            .await
            .expect("list events")
            .into_iter()
            .map(fields)
            .collect::<Vec<_>>();

        assert_eq!(
            (deleted, rows),
            (
                1,
                vec![
                    (
                        "jquants".into(),
                        "outside".into(),
                        "outside range".into(),
                        date(2099, 9, 1),
                    ),
                    (
                        "jquants".into(),
                        "past".into(),
                        "past event".into(),
                        date(2099, 8, 9),
                    ),
                    (
                        "other".into(),
                        "other-source".into(),
                        "other source".into(),
                        date(2099, 8, 11),
                    ),
                ],
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn upsert_chunks_large_event_batches(db: DatabaseHandle) {
        let repository = PostgresCalendarEventRepository::new(db.clone());
        let unit_of_work = PostgresUnitOfWork::new(db.clone());
        let transaction = unit_of_work.begin().await.expect("begin transaction");
        let events = (0..6_001)
            .map(|index| {
                event(
                    "alpha_vantage_fixture",
                    &format!("fixture-{index}"),
                    "Example Holdings",
                    date(2099, 8, 11),
                )
            })
            .collect();

        let upserted = repository
            .upsert(&transaction, events)
            .await
            .expect("upsert large batch");
        unit_of_work
            .commit(transaction)
            .await
            .expect("commit transaction");
        let rows = calendar_event::Entity::find()
            .filter(calendar_event::Column::Source.eq("alpha_vantage_fixture"))
            .all(&db)
            .await
            .expect("list inserted events");

        assert_eq!((upserted, rows.len()), (6_001, 6_001));
    }

    #[backend_test_macros::database_test]
    async fn enrich_events_returns_references_and_only_exact_actual_summaries(db: DatabaseHandle) {
        let repository = PostgresCalendarEventRepository::new(db.clone());
        let financial_summary_repository = PostgresFinancialSummaryRepository::new(db.clone());
        let unit_of_work = PostgresUnitOfWork::new(db);
        let period_end = date(2042, 6, 30);
        let matching_event = jp_event("FAKE", "2Q", period_end);
        let forecast_only_event = jp_event("FICT", "2Q", period_end);
        let prefix_collision_event = jp_event("DUPL", "2Q", period_end);
        let mut unsupported_source_event = matching_event.clone();
        unsupported_source_event.source = "sample_source".into();
        let references = vec![
            CalendarEventReference {
                event_source: matching_event.source.clone(),
                event_external_id: matching_event.external_id.clone(),
                kind: CalendarEventReferenceKind::IndividualDocument,
                source: "sample_source".into(),
                external_id: "sample-document".into(),
                title: "架空の決算資料".into(),
                url: Some("https://example.invalid/sample-document".into()),
                target_period_start: None,
                target_period_end: Some(period_end),
                target_period_type: Some("2Q".into()),
                publication_evidence: Some("sample-release-record".into()),
                publication_evidence_url: Some("https://example.invalid/sample-evidence".into()),
                checked_at: DateTime::<Utc>::from_timestamp(1_700_000_000, 0)
                    .expect("valid timestamp"),
            },
            CalendarEventReference {
                event_source: matching_event.source.clone(),
                event_external_id: matching_event.external_id.clone(),
                kind: CalendarEventReferenceKind::OfficialListing,
                source: "sample_source".into(),
                external_id: "sample-listing".into(),
                title: "架空の公式一覧".into(),
                url: None,
                target_period_start: None,
                target_period_end: Some(period_end),
                target_period_type: Some("2Q".into()),
                publication_evidence: None,
                publication_evidence_url: None,
                checked_at: DateTime::<Utc>::from_timestamp(1_700_000_100, 0)
                    .expect("valid timestamp"),
            },
        ];
        let transaction = unit_of_work.begin().await.expect("begin transaction");
        repository
            .upsert(
                &transaction,
                vec![
                    matching_event.clone(),
                    forecast_only_event.clone(),
                    prefix_collision_event.clone(),
                    unsupported_source_event.clone(),
                ],
            )
            .await
            .expect("upsert events");
        repository
            .upsert_references(&transaction, references.clone())
            .await
            .expect("upsert references");
        financial_summary_repository
            .upsert(
                &transaction,
                vec![
                    period_summary(
                        "FAKE0",
                        "1001",
                        "sample-actual-group",
                        "2QFinancialStatements_Sample",
                        "2Q",
                        period_end,
                    ),
                    period_summary(
                        "FICT0",
                        "1002",
                        "sample-forecast-group",
                        "EarnForecastRevision",
                        "2Q",
                        period_end,
                    ),
                    period_summary(
                        "DUPL1",
                        "1003",
                        "sample-prefix-collision",
                        "2QFinancialStatements_Sample",
                        "2Q",
                        period_end,
                    ),
                ],
            )
            .await
            .expect("upsert financial summaries");
        unit_of_work
            .commit(transaction)
            .await
            .expect("commit transaction");

        let actual = repository
            .enrich_events(
                repository
                    .list_events(&DateRange {
                        from: date(2042, 8, 1),
                        to: date(2042, 8, 1),
                    })
                    .await
                    .expect("list events"),
            )
            .await
            .expect("enrich events");
        let expected = vec![
            CalendarEventReadModel {
                event: CalendarEvent {
                    stock_id: Some("DUPL".into()),
                    ..prefix_collision_event
                },
                references: Vec::new(),
                has_matching_earnings_summary: false,
            },
            CalendarEventReadModel {
                event: CalendarEvent {
                    stock_id: Some("FAKE".into()),
                    ..matching_event.clone()
                },
                references: references.clone(),
                has_matching_earnings_summary: true,
            },
            CalendarEventReadModel {
                event: CalendarEvent {
                    stock_id: Some("FICT".into()),
                    ..forecast_only_event
                },
                references: Vec::new(),
                has_matching_earnings_summary: false,
            },
            CalendarEventReadModel {
                event: CalendarEvent {
                    stock_id: Some("FAKE".into()),
                    ..unsupported_source_event
                },
                references: Vec::new(),
                has_matching_earnings_summary: false,
            },
        ];
        let listed_references = repository
            .list_references_for_event(&matching_event.source, &matching_event.external_id)
            .await
            .expect("list event references");

        assert_eq!((actual, listed_references), (expected, references));
    }
}
