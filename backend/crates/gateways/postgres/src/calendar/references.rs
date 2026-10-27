use chrono::{DateTime, FixedOffset, NaiveDate, Utc};
use core_application::calendar::repository::CalendarEventRepositoryError;
use core_application::sec_filing_reference::SecFilingReferenceTarget;
use core_application::unit_of_work::UnitOfWorkTransaction;
use core_domain::calendar_event::{
    CalendarEvent, CalendarEventCategory, CalendarEventReadModel, CalendarEventReference,
    CalendarEventReferenceKind,
};
use core_domain::financial_summary::{
    FinancialSummaryPeriodType, is_actual_financial_summary_for_period,
};
use sea_orm::ActiveValue::Set;
use sea_orm::sea_query::OnConflict;
use sea_orm::{
    ColumnTrait, Condition, ConnectionTrait, DatabaseBackend, EntityTrait, FromQueryResult,
    Iterable, QueryFilter, QueryOrder, Statement, Value,
};
use std::collections::{HashMap, HashSet};

use super::{
    MAX_UPSERT_ROWS_PER_STATEMENT, PostgresCalendarEventRepository, normalize_local_code,
    repository_error, transaction_ref,
};
use crate::entities::{calendar_event, calendar_event_reference};

const MAX_CONTEXT_ROWS_PER_QUERY: usize = 1_000;
const MAX_PENDING_SEC_FILING_TARGETS: i64 = 500;

#[derive(Debug, FromQueryResult)]
struct CalendarEventReferenceRow {
    event_source: String,
    event_external_id: String,
    kind: String,
    source: String,
    external_id: String,
    title: String,
    url: Option<String>,
    target_period_start: Option<NaiveDate>,
    target_period_end: Option<NaiveDate>,
    target_period_type: Option<String>,
    publication_evidence: Option<String>,
    publication_evidence_url: Option<String>,
    checked_at: DateTime<FixedOffset>,
}

impl CalendarEventReferenceRow {
    fn from_model(model: calendar_event_reference::Model) -> Self {
        Self {
            event_source: String::new(),
            event_external_id: String::new(),
            kind: model.kind,
            source: model.source,
            external_id: model.external_id,
            title: model.title,
            url: model.url,
            target_period_start: model.target_period_start,
            target_period_end: model.target_period_end,
            target_period_type: model.target_period_type,
            publication_evidence: model.publication_evidence,
            publication_evidence_url: model.publication_evidence_url,
            checked_at: model.checked_at,
        }
    }
}

#[derive(Debug, FromQueryResult)]
struct FinancialSummaryMatchRow {
    event_source: String,
    event_external_id: String,
    document_type: Option<String>,
    current_period_type: Option<String>,
    current_period_end: Option<NaiveDate>,
}

#[derive(Debug, FromQueryResult)]
struct SecFilingTargetRow {
    event_source: String,
    event_external_id: String,
    stock_id: String,
    cik: String,
    fiscal_period: String,
}

#[derive(Debug)]
struct EarningsPeriodTarget {
    event_source: String,
    event_external_id: String,
    stock_id: String,
    period_end: NaiveDate,
    period_type: FinancialSummaryPeriodType,
}

pub(super) async fn enrich_events(
    repository: &PostgresCalendarEventRepository,
    events: Vec<CalendarEvent>,
) -> Result<Vec<CalendarEventReadModel>, CalendarEventRepositoryError> {
    if events.is_empty() {
        return Ok(Vec::new());
    }

    let mut references_by_event: HashMap<(String, String), Vec<CalendarEventReference>> =
        HashMap::new();
    for events_chunk in events.chunks(MAX_CONTEXT_ROWS_PER_QUERY) {
        let (event_filter, event_values) = event_key_filter(
            events_chunk
                .iter()
                .map(|event| (event.source.as_str(), event.external_id.as_str())),
        );
        let reference_sql = format!(
            "SELECT e.source AS event_source, e.external_id AS event_external_id, \
             r.kind, r.source, r.external_id, r.title, r.url, r.target_period_start, \
             r.target_period_end, r.target_period_type, r.publication_evidence, \
             r.publication_evidence_url, r.checked_at \
             FROM calendar_event_reference r \
             JOIN calendar_event e ON e.id = r.calendar_event_id \
             WHERE {event_filter} \
             ORDER BY e.source, e.external_id, r.kind, r.source, r.external_id"
        );
        let reference_rows = repository
            .db
            .query_all_raw(Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                reference_sql,
                event_values,
            ))
            .await
            .map_err(repository_error)?;

        for row in &reference_rows {
            let row =
                CalendarEventReferenceRow::from_query_result(row, "").map_err(repository_error)?;
            let key = (row.event_source.clone(), row.event_external_id.clone());
            references_by_event
                .entry(key.clone())
                .or_default()
                .push(to_reference_domain(row, key.0, key.1)?);
        }
    }

    let targets = earnings_period_targets(&events);
    let mut matching_event_keys = HashSet::new();
    for targets_chunk in targets.chunks(MAX_CONTEXT_ROWS_PER_QUERY) {
        matching_event_keys.extend(find_matching_earnings_events(repository, targets_chunk).await?);
    }
    Ok(events
        .into_iter()
        .map(|event| {
            let key = (event.source.clone(), event.external_id.clone());
            CalendarEventReadModel {
                event,
                references: references_by_event.remove(&key).unwrap_or_default(),
                has_matching_earnings_summary: matching_event_keys.contains(&key),
            }
        })
        .collect())
}

pub(super) async fn list_references_for_event(
    repository: &PostgresCalendarEventRepository,
    event_source: &str,
    event_external_id: &str,
) -> Result<Vec<CalendarEventReference>, CalendarEventRepositoryError> {
    let event = calendar_event::Entity::find()
        .filter(calendar_event::Column::Source.eq(event_source))
        .filter(calendar_event::Column::ExternalId.eq(event_external_id))
        .one(&repository.db)
        .await
        .map_err(repository_error)?
        .ok_or(CalendarEventRepositoryError::MissingEvent)?;

    calendar_event_reference::Entity::find()
        .filter(calendar_event_reference::Column::CalendarEventId.eq(event.id))
        .order_by_asc(calendar_event_reference::Column::Kind)
        .order_by_asc(calendar_event_reference::Column::Source)
        .order_by_asc(calendar_event_reference::Column::ExternalId)
        .all(&repository.db)
        .await
        .map_err(repository_error)?
        .into_iter()
        .map(|reference| {
            to_reference_domain(
                CalendarEventReferenceRow::from_model(reference),
                event_source.to_owned(),
                event_external_id.to_owned(),
            )
        })
        .collect()
}

pub(super) async fn list_pending_sec_filing_targets(
    repository: &PostgresCalendarEventRepository,
    today: NaiveDate,
) -> Result<Vec<SecFilingReferenceTarget>, CalendarEventRepositoryError> {
    let rows = repository
        .db
        .query_all_raw(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            "SELECT e.source AS event_source, e.external_id AS event_external_id, \
                    s.id AS stock_id, s.cik AS cik, e.fiscal_period AS fiscal_period \
             FROM calendar_event e \
             JOIN stock s ON s.id = e.stock_id \
             LEFT JOIN calendar_event_sec_filing_check c ON c.calendar_event_id = e.id \
             WHERE e.source = 'alpha_vantage' AND e.country = 'US' \
               AND e.category = 'earnings' \
               AND e.event_date <= $1 AND e.fiscal_period IS NOT NULL \
               AND e.stock_id LIKE 'US:%' \
               AND s.cik ~ '^[0-9]{1,10}$' AND s.cik !~ '^0+$' \
               AND NOT EXISTS ( \
                   SELECT 1 FROM calendar_event_reference r \
                   WHERE r.calendar_event_id = e.id \
                     AND r.kind = 'individual_document' AND r.source = 'sec' \
               ) \
             ORDER BY c.checked_at ASC NULLS FIRST, e.event_date DESC, e.source, e.external_id \
             LIMIT $2",
            [today.into(), MAX_PENDING_SEC_FILING_TARGETS.into()],
        ))
        .await
        .map_err(repository_error)?;

    rows.iter()
        .map(|row| {
            let row = SecFilingTargetRow::from_query_result(row, "").map_err(repository_error)?;
            Ok(SecFilingReferenceTarget {
                event_source: row.event_source,
                event_external_id: row.event_external_id,
                stock_id: row.stock_id,
                cik: row.cik,
                fiscal_period: row.fiscal_period,
            })
        })
        .collect()
}

pub(super) async fn mark_sec_filing_targets_checked(
    repository: &PostgresCalendarEventRepository,
    targets: &[SecFilingReferenceTarget],
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
            "INSERT INTO calendar_event_sec_filing_check (calendar_event_id, checked_at) \
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

pub(super) async fn upsert_references(
    transaction: &UnitOfWorkTransaction,
    references: Vec<CalendarEventReference>,
) -> Result<usize, CalendarEventRepositoryError> {
    if references.is_empty() {
        return Ok(0);
    }

    let transaction = transaction_ref(transaction)?;
    let mut event_conditions = Condition::any();
    let mut event_keys = HashSet::new();
    for reference in &references {
        let key = (
            reference.event_source.clone(),
            reference.event_external_id.clone(),
        );
        if event_keys.insert(key.clone()) {
            event_conditions = event_conditions.add(
                Condition::all()
                    .add(calendar_event::Column::Source.eq(key.0))
                    .add(calendar_event::Column::ExternalId.eq(key.1)),
            );
        }
    }

    let events = calendar_event::Entity::find()
        .filter(event_conditions)
        .all(transaction)
        .await
        .map_err(repository_error)?;
    let event_ids = events
        .into_iter()
        .map(|event| ((event.source, event.external_id), event.id))
        .collect::<HashMap<_, _>>();
    if event_keys.iter().any(|key| !event_ids.contains_key(key)) {
        return Err(CalendarEventRepositoryError::MissingEvent);
    }

    let count = references.len();
    let updated_at = Utc::now().fixed_offset();
    let models = references
        .into_iter()
        .map(|reference| to_reference_active_model(reference, &event_ids, updated_at))
        .collect::<Result<Vec<_>, _>>()?;
    let columns_per_row = calendar_event_reference::Column::iter().count();
    let chunk_size = crate::repositories::max_rows_per_statement(columns_per_row)
        .min(MAX_UPSERT_ROWS_PER_STATEMENT);

    for chunk in models.chunks(chunk_size) {
        calendar_event_reference::Entity::insert_many(chunk.to_vec())
            .on_conflict(
                OnConflict::columns([
                    calendar_event_reference::Column::CalendarEventId,
                    calendar_event_reference::Column::Source,
                    calendar_event_reference::Column::ExternalId,
                ])
                .update_columns([
                    calendar_event_reference::Column::Kind,
                    calendar_event_reference::Column::Title,
                    calendar_event_reference::Column::Url,
                    calendar_event_reference::Column::TargetPeriodStart,
                    calendar_event_reference::Column::TargetPeriodEnd,
                    calendar_event_reference::Column::TargetPeriodType,
                    calendar_event_reference::Column::PublicationEvidence,
                    calendar_event_reference::Column::PublicationEvidenceUrl,
                    calendar_event_reference::Column::CheckedAt,
                    calendar_event_reference::Column::UpdatedAt,
                ])
                .to_owned(),
            )
            .exec_without_returning(transaction)
            .await
            .map_err(repository_error)?;
    }

    Ok(count)
}

fn event_key_filter<'a>(keys: impl Iterator<Item = (&'a str, &'a str)>) -> (String, Vec<Value>) {
    let mut conditions = Vec::new();
    let mut values = Vec::new();
    let mut seen = HashSet::new();
    for (source, external_id) in keys {
        let key = (source.to_owned(), external_id.to_owned());
        if !seen.insert(key.clone()) {
            continue;
        }
        let source_param = values.len() + 1;
        let external_id_param = source_param + 1;
        conditions.push(format!(
            "(e.source = ${source_param} AND e.external_id = ${external_id_param})"
        ));
        values.extend([key.0.into(), key.1.into()]);
    }

    if conditions.is_empty() {
        ("FALSE".into(), values)
    } else {
        (conditions.join(" OR "), values)
    }
}

fn earnings_period_targets(events: &[CalendarEvent]) -> Vec<EarningsPeriodTarget> {
    events
        .iter()
        .filter_map(|event| {
            if event.category != CalendarEventCategory::Earnings
                || event.country != "JP"
                || event.source != core_application::earnings_schedule::JQUANTS_EARNINGS_SOURCE
            {
                return None;
            }
            let stock_id = event.stock_id.as_deref()?;
            let fiscal_period = event.fiscal_period.as_deref()?;
            let mut parts = event.external_id.split(':');
            let code = parts.next()?;
            let period_type = parts.next()?;
            let external_period = parts.next()?;
            if parts.next().is_some()
                || normalize_local_code(code) != stock_id
                || external_period != fiscal_period
            {
                return None;
            }
            let period_type = FinancialSummaryPeriodType::parse(period_type)?;
            let period_end = NaiveDate::parse_from_str(fiscal_period, "%Y-%m-%d").ok()?;
            Some(EarningsPeriodTarget {
                event_source: event.source.clone(),
                event_external_id: event.external_id.clone(),
                stock_id: stock_id.to_owned(),
                period_end,
                period_type,
            })
        })
        .collect()
}

async fn find_matching_earnings_events(
    repository: &PostgresCalendarEventRepository,
    targets: &[EarningsPeriodTarget],
) -> Result<HashSet<(String, String)>, CalendarEventRepositoryError> {
    if targets.is_empty() {
        return Ok(HashSet::new());
    }

    let mut values = Vec::with_capacity(targets.len() * 5);
    let mut target_values = Vec::with_capacity(targets.len());
    for target in targets {
        let first_param = values.len() + 1;
        target_values.push(format!(
            "(${first_param}, ${}, ${}, ${}::date, ${})",
            first_param + 1,
            first_param + 2,
            first_param + 3,
            first_param + 4,
        ));
        values.extend([
            target.event_source.clone().into(),
            target.event_external_id.clone().into(),
            target.stock_id.clone().into(),
            target.period_end.into(),
            target.period_type.as_str().into(),
        ]);
    }

    let sql = format!(
        "WITH target_events(event_source, event_external_id, stock_id, period_end, period_type) AS (VALUES {}) \
         SELECT target_events.event_source, target_events.event_external_id, \
                financial_summary.document_type, financial_summary.current_period_type, \
                financial_summary.current_period_end \
         FROM target_events \
         JOIN financial_summary ON (financial_summary.code = target_events.stock_id \
               OR (char_length(target_events.stock_id) = 4 AND \
                   financial_summary.code = target_events.stock_id || '0')) \
           AND financial_summary.current_period_end = target_events.period_end \
           AND financial_summary.current_period_type = target_events.period_type",
        target_values.join(", "),
    );
    let rows = repository
        .db
        .query_all_raw(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            sql,
            values,
        ))
        .await
        .map_err(repository_error)?;

    let target_by_key = targets
        .iter()
        .map(|target| {
            (
                (
                    target.event_source.clone(),
                    target.event_external_id.clone(),
                ),
                (target.period_type, target.period_end),
            )
        })
        .collect::<HashMap<_, _>>();
    let mut matches = HashSet::new();
    for row in rows {
        let row =
            FinancialSummaryMatchRow::from_query_result(&row, "").map_err(repository_error)?;
        let key = (row.event_source, row.event_external_id);
        let Some((period_type, period_end)) = target_by_key.get(&key) else {
            continue;
        };
        if is_actual_financial_summary_for_period(
            row.document_type.as_deref(),
            row.current_period_type.as_deref(),
            row.current_period_end,
            *period_type,
            *period_end,
        ) {
            matches.insert(key);
        }
    }

    Ok(matches)
}

fn to_reference_domain(
    row: CalendarEventReferenceRow,
    event_source: String,
    event_external_id: String,
) -> Result<CalendarEventReference, CalendarEventRepositoryError> {
    let kind = CalendarEventReferenceKind::parse(&row.kind)
        .ok_or(CalendarEventRepositoryError::InvalidReference)?;
    Ok(CalendarEventReference {
        event_source,
        event_external_id,
        kind,
        source: row.source,
        external_id: row.external_id,
        title: row.title,
        url: row.url,
        target_period_start: row.target_period_start,
        target_period_end: row.target_period_end,
        target_period_type: row.target_period_type,
        publication_evidence: row.publication_evidence,
        publication_evidence_url: row.publication_evidence_url,
        checked_at: row.checked_at.with_timezone(&Utc),
    })
}

fn to_reference_active_model(
    reference: CalendarEventReference,
    event_ids: &HashMap<(String, String), uuid::Uuid>,
    updated_at: DateTime<FixedOffset>,
) -> Result<calendar_event_reference::ActiveModel, CalendarEventRepositoryError> {
    let key = (
        reference.event_source.clone(),
        reference.event_external_id.clone(),
    );
    let event_id = event_ids
        .get(&key)
        .copied()
        .ok_or(CalendarEventRepositoryError::MissingEvent)?;
    Ok(calendar_event_reference::ActiveModel {
        id: Set(uuid::Uuid::new_v4()),
        calendar_event_id: Set(event_id),
        kind: Set(reference.kind.as_str().to_owned()),
        source: Set(reference.source),
        external_id: Set(reference.external_id),
        title: Set(reference.title),
        url: Set(reference.url),
        target_period_start: Set(reference.target_period_start),
        target_period_end: Set(reference.target_period_end),
        target_period_type: Set(reference.target_period_type),
        publication_evidence: Set(reference.publication_evidence),
        publication_evidence_url: Set(reference.publication_evidence_url),
        checked_at: Set(reference.checked_at.fixed_offset()),
        created_at: Set(updated_at),
        updated_at: Set(updated_at),
    })
}
