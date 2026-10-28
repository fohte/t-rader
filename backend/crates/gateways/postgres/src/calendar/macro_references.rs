use chrono::{DateTime, NaiveDate, Utc};
use core_application::calendar::macro_reference_targets::{
    MacroReferenceTarget, MacroReferenceTargetSelector,
};
use core_application::calendar::repository::CalendarEventRepositoryError;
use sea_orm::{ConnectionTrait, DatabaseBackend, FromQueryResult, Statement};

use super::{PostgresCalendarEventRepository, repository_error};

const MAX_PENDING_MACRO_REFERENCE_TARGETS: i64 = 500;
#[derive(Debug, FromQueryResult)]
struct MacroReferenceTargetRow {
    event_source: String,
    event_external_id: String,
    event_title: String,
    same_day_event_count: i64,
}

pub(super) async fn list_macro_reference_targets(
    repository: &PostgresCalendarEventRepository,
    today: NaiveDate,
    selector: &MacroReferenceTargetSelector,
) -> Result<Vec<MacroReferenceTarget>, CalendarEventRepositoryError> {
    let statement = if selector.include_future {
        Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            "SELECT e.source AS event_source, e.external_id AS event_external_id, e.title AS event_title, \
                    COUNT(*) OVER (PARTITION BY e.source, e.country, split_part(e.external_id, ':', 1), e.event_date) \
                      AS same_day_event_count \
             FROM calendar_event e \
             LEFT JOIN calendar_event_macro_reference_check c ON c.calendar_event_id = e.id \
             WHERE e.category = $1 AND e.source = $2 AND e.country = $3 AND e.external_id ~ $4 \
             ORDER BY c.checked_at ASC NULLS FIRST, e.event_date DESC, e.source, e.external_id \
             LIMIT $5",
            [
                selector.category.clone().into(),
                selector.event_source.clone().into(),
                selector.country.clone().into(),
                selector.external_id_pattern.clone().into(),
                MAX_PENDING_MACRO_REFERENCE_TARGETS.into(),
            ],
        )
    } else {
        Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            "SELECT e.source AS event_source, e.external_id AS event_external_id, e.title AS event_title, \
                    COUNT(*) OVER (PARTITION BY e.source, e.country, split_part(e.external_id, ':', 1), e.event_date) \
                      AS same_day_event_count \
             FROM calendar_event e \
             LEFT JOIN calendar_event_macro_reference_check c ON c.calendar_event_id = e.id \
             WHERE e.event_date < $1 AND e.category = $2 \
               AND e.source = $3 AND e.country = $4 AND e.external_id ~ $5 \
             ORDER BY c.checked_at ASC NULLS FIRST, e.event_date DESC, e.source, e.external_id \
             LIMIT $6",
            [
                today.into(),
                selector.category.clone().into(),
                selector.event_source.clone().into(),
                selector.country.clone().into(),
                selector.external_id_pattern.clone().into(),
                MAX_PENDING_MACRO_REFERENCE_TARGETS.into(),
            ],
        )
    };
    let rows = repository
        .db
        .query_all_raw(statement)
        .await
        .map_err(repository_error)?;

    rows.iter()
        .map(|row| {
            let row =
                MacroReferenceTargetRow::from_query_result(row, "").map_err(repository_error)?;
            Ok(MacroReferenceTarget {
                event_source: row.event_source,
                event_external_id: row.event_external_id,
                event_title: row.event_title,
                same_day_event_count: row.same_day_event_count,
            })
        })
        .collect()
}

pub(super) async fn mark_macro_reference_targets_checked(
    repository: &PostgresCalendarEventRepository,
    targets: &[MacroReferenceTarget],
    checked_at: DateTime<Utc>,
) -> Result<(), CalendarEventRepositoryError> {
    if targets.is_empty() {
        return Ok(());
    }
    let targets_json = serde_json::json!(
        targets
            .iter()
            .map(|target| serde_json::json!({
                "event_source": target.event_source,
                "event_external_id": target.event_external_id,
            }))
            .collect::<Vec<_>>()
    );
    repository
        .db
        .execute_raw(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            "INSERT INTO calendar_event_macro_reference_check (calendar_event_id, checked_at) \
             SELECT e.id, $1 \
             FROM calendar_event e \
             JOIN jsonb_to_recordset($2::jsonb) AS target(event_source text, event_external_id text) \
               ON e.source = target.event_source AND e.external_id = target.event_external_id \
             ON CONFLICT (calendar_event_id) DO UPDATE \
             SET checked_at = EXCLUDED.checked_at",
            [checked_at.into(), targets_json.into()],
        ))
        .await
        .map_err(repository_error)?;
    Ok(())
}
