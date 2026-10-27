use std::sync::Arc;

use chrono::{DateTime, NaiveDate, TimeZone, Utc};
use chrono_tz::Asia::Tokyo;
use core_application::calendar::macro_reference_targets::{
    MacroReferenceTarget, MacroReferenceTargetRepository, MacroReferenceTargetSelector,
};
use core_application::calendar::{
    references::CalendarEventReferenceUseCases,
    repository::{CalendarEventRepository, SharedCalendarEventRepository},
};
use core_application::unit_of_work::UnitOfWork;
use core_domain::calendar_event::{
    CalendarEvent, CalendarEventCategory, CalendarEventReference, CalendarEventReferenceKind,
};
use gateway_postgres::{
    PostgresCalendarEventRepository, PostgresUnitOfWork,
    entities::calendar_event_macro_reference_check,
};
use sea_orm::{EntityTrait, QueryOrder};

use crate::integration_tests::calendar_test_support::date;

fn indicator_event(
    source: &str,
    external_id: &str,
    country: &str,
    event_date: NaiveDate,
    event_at: Option<DateTime<Utc>>,
) -> CalendarEvent {
    CalendarEvent {
        source: source.to_owned(),
        external_id: external_id.to_owned(),
        category: CalendarEventCategory::Indicator,
        country: country.to_owned(),
        title: "架空の統計イベント".to_owned(),
        stock_id: None,
        fiscal_period: None,
        event_date,
        event_at,
        time_of_day: None,
    }
}

#[backend_test_macros::database_test]
async fn macro_reference_query_excludes_today_and_selects_previous_day_events(
    db: gateway_postgres::DatabaseHandle,
) -> Result<(), Box<dyn std::error::Error>> {
    let today_release_at = Tokyo
        .from_local_datetime(
            &date(2099, 4, 9)
                .and_hms_opt(8, 30, 0)
                .ok_or_else(|| std::io::Error::other("synthetic local time is out of range"))?,
        )
        .single()
        .map(|date_time| date_time.with_timezone(&Utc))
        .ok_or_else(|| std::io::Error::other("synthetic local time is ambiguous"))?;
    let repository = PostgresCalendarEventRepository::new(db.clone());
    let unit_of_work = PostgresUnitOfWork::new(db.clone());
    let transaction = unit_of_work.begin().await?;
    repository
        .upsert(
            &transaction,
            vec![
                indicator_event(
                    "e_stat",
                    "sample-stat-code:209904080830",
                    "JP",
                    date(2099, 4, 8),
                    None,
                ),
                indicator_event(
                    "e_stat",
                    "sample-stat-code:209904081030",
                    "JP",
                    date(2099, 4, 8),
                    None,
                ),
                indicator_event(
                    "e_stat",
                    "sample-stat-code:209904090830",
                    "JP",
                    date(2099, 4, 9),
                    Some(today_release_at),
                ),
                indicator_event(
                    "e_stat",
                    "unrelated-stat-code:209904080830",
                    "JP",
                    date(2099, 4, 8),
                    None,
                ),
                indicator_event(
                    "e_stat",
                    "sample-stat-code:209904130830",
                    "JP",
                    date(2099, 4, 13),
                    None,
                ),
                indicator_event(
                    "fred",
                    "sample-release:2099-04-08",
                    "US",
                    date(2099, 4, 8),
                    None,
                ),
                indicator_event(
                    "fred",
                    "sample-release:2099-04-09",
                    "US",
                    date(2099, 4, 9),
                    None,
                ),
                indicator_event(
                    "fred",
                    "unrelated-release:2099-04-08",
                    "US",
                    date(2099, 4, 8),
                    None,
                ),
            ],
        )
        .await?;
    unit_of_work.commit(transaction).await?;

    let e_stat_selector =
        MacroReferenceTargetSelector::new("e_stat", "JP", r"^sample-stat-code:[0-9]{12}$");
    let fred_selector = MacroReferenceTargetSelector::new(
        "fred",
        "US",
        r"^sample-release:[0-9]{4}-[0-9]{2}-[0-9]{2}$",
    );
    let e_stat_targets = repository
        .list_pending_macro_reference_targets(date(2099, 4, 9), &e_stat_selector)
        .await?;
    let fred_targets = repository
        .list_pending_macro_reference_targets(date(2099, 4, 9), &fred_selector)
        .await?;
    let checked_at = DateTime::from_timestamp(4_075_555_200, 0)
        .ok_or_else(|| std::io::Error::other("synthetic check timestamp is out of range"))?;
    let expected_checked_at = DateTime::from_timestamp(4_075_555_200, 0)
        .ok_or_else(|| std::io::Error::other("synthetic check timestamp is out of range"))?;
    repository
        .mark_macro_reference_targets_checked(&e_stat_targets, checked_at)
        .await?;
    let checks = calendar_event_macro_reference_check::Entity::find()
        .order_by_asc(calendar_event_macro_reference_check::Column::CheckedAt)
        .all(&db)
        .await?;

    assert_eq!(
        (
            e_stat_targets,
            fred_targets,
            checks
                .into_iter()
                .map(|check| check.checked_at.with_timezone(&Utc))
                .collect::<Vec<_>>(),
        ),
        (
            vec![
                MacroReferenceTarget {
                    event_source: "e_stat".to_owned(),
                    event_external_id: "sample-stat-code:209904080830".to_owned(),
                    event_title: "架空の統計イベント".to_owned(),
                    same_day_event_count: 2,
                },
                MacroReferenceTarget {
                    event_source: "e_stat".to_owned(),
                    event_external_id: "sample-stat-code:209904081030".to_owned(),
                    event_title: "架空の統計イベント".to_owned(),
                    same_day_event_count: 2,
                }
            ],
            vec![MacroReferenceTarget {
                event_source: "fred".to_owned(),
                event_external_id: "sample-release:2099-04-08".to_owned(),
                event_title: "架空の統計イベント".to_owned(),
                same_day_event_count: 1,
            }],
            vec![expected_checked_at, expected_checked_at],
        ),
    );
    Ok(())
}

#[backend_test_macros::database_test]
async fn macro_reference_refresh_replaces_outdated_documents_with_the_listing(
    db: gateway_postgres::DatabaseHandle,
) -> Result<(), Box<dyn std::error::Error>> {
    let repository = Arc::new(PostgresCalendarEventRepository::new(db.clone()));
    let unit_of_work = Arc::new(PostgresUnitOfWork::new(db));
    let event = indicator_event(
        "e_stat",
        "sample-stat-code:209904080830",
        "JP",
        date(2099, 4, 8),
        None,
    );
    let event_key = (event.source.clone(), event.external_id.clone());
    let checked_at = DateTime::from_timestamp(1_735_689_600, 0)
        .ok_or_else(|| std::io::Error::other("synthetic check timestamp is out of range"))?;
    let previous_document = CalendarEventReference {
        event_source: event.source.clone(),
        event_external_id: event.external_id.clone(),
        kind: CalendarEventReferenceKind::IndividualDocument,
        source: "sample_source".to_owned(),
        external_id: "sample-document".to_owned(),
        title: "架空の個別資料".to_owned(),
        url: Some("https://example.invalid/sample-document".to_owned()),
        target_period_start: Some(date(2099, 1, 1)),
        target_period_end: Some(date(2099, 1, 31)),
        target_period_type: Some("month".to_owned()),
        publication_evidence: Some("架空の公表記録".to_owned()),
        publication_evidence_url: Some("https://example.invalid/sample-evidence".to_owned()),
        checked_at,
    };
    let updated_listing = CalendarEventReference {
        event_source: event.source.clone(),
        event_external_id: event.external_id.clone(),
        kind: CalendarEventReferenceKind::OfficialListing,
        source: "sample_source".to_owned(),
        external_id: "sample-official-listing".to_owned(),
        title: "架空の公式一覧".to_owned(),
        url: Some("https://example.invalid/sample-listing".to_owned()),
        target_period_start: None,
        target_period_end: None,
        target_period_type: None,
        publication_evidence: None,
        publication_evidence_url: None,
        checked_at,
    };

    let transaction = unit_of_work.begin().await?;
    repository.upsert(&transaction, vec![event]).await?;
    repository
        .upsert_references(&transaction, vec![previous_document])
        .await?;
    unit_of_work.commit(transaction).await?;

    let use_cases = CalendarEventReferenceUseCases::new(
        repository.clone() as SharedCalendarEventRepository,
        unit_of_work,
    );
    let actual = (
        use_cases
            .replace_macro_references_for_events(
                vec![event_key.clone()],
                vec![updated_listing.clone()],
            )
            .await?,
        use_cases.list_for_event(&event_key.0, &event_key.1).await?,
    );

    assert_eq!(actual, (1, vec![updated_listing]));
    Ok(())
}
