use async_trait::async_trait;
use chrono::NaiveDate;
use core_application::calendar::repository::CalendarEventRepository;
use core_application::sec_filing_source::{
    SecFilingDocument, SecFilingSource, SecFilingSourceError,
};
use core_application::unit_of_work::UnitOfWork;
use core_application::us_stock_master_source::{UsStockMasterSource, UsStockMasterSourceError};
use core_domain::calendar_event::{CalendarEvent, CalendarEventCategory, CalendarEventTimeOfDay};
use core_domain::stock_id::ForeignStockId;
use core_domain::us_stock_master::UsStockMasterEntry;
use gateway_postgres::{PostgresCalendarEventRepository, PostgresUnitOfWork};
use sea_orm::{ActiveModelTrait, ActiveValue::Set};
use serde_json::{Value, json};

use crate::{
    integration_tests::calendar_test_support::date,
    services::use_cases::build_use_cases_without_agent_task_client,
    testing::{create_test_server, insert_test_strategy},
};

struct FixedUsStockMasterSource;

#[async_trait]
impl UsStockMasterSource for FixedUsStockMasterSource {
    async fn fetch_all_us_stocks(
        &self,
    ) -> Result<Vec<UsStockMasterEntry>, UsStockMasterSourceError> {
        Ok(vec![
            stock_entry("QZ7", "架空銘柄 A", Some("900000001")),
            stock_entry("QZ8", "架空銘柄 B", Some("900000002")),
            stock_entry("QZ9", "架空銘柄 C", None),
        ])
    }
}

struct FixedSecFilingSource;

#[async_trait]
impl SecFilingSource for FixedSecFilingSource {
    async fn fetch_periodic_reports(
        &self,
        cik: &str,
        _ticker: &str,
        report_dates: &[NaiveDate],
    ) -> Result<Vec<SecFilingDocument>, SecFilingSourceError> {
        let Some(report_date) = report_dates.first().copied() else {
            return Ok(Vec::new());
        };
        let padded_cik = format!("{cik:0>10}");
        let accession_number = format!("{padded_cik}-99-000001");
        let accession_path = accession_number.replace('-', "");
        Ok(vec![SecFilingDocument {
            form: "10-Q".to_owned(),
            report_date,
            accession_number,
            primary_document: "report.htm".to_owned(),
            url: format!(
                "https://example.invalid/Archives/edgar/data/{cik}/{accession_path}/report.htm"
            ),
        }])
    }
}

fn stock_entry(code: &str, name: &str, cik: Option<&str>) -> UsStockMasterEntry {
    UsStockMasterEntry {
        id: ForeignStockId::new("US", code).expect("valid synthetic stock id"),
        name: name.to_owned(),
        exchange: Some("Example Exchange".to_owned()),
        cik: cik.map(str::to_owned),
    }
}

fn us_earnings_event(code: &str, title: &str) -> CalendarEvent {
    CalendarEvent {
        source: "alpha_vantage".to_owned(),
        external_id: format!("US:{code}:2031-03-30"),
        category: CalendarEventCategory::Earnings,
        country: "US".to_owned(),
        title: title.to_owned(),
        stock_id: Some(format!("US:{code}")),
        fiscal_period: Some("2031-03-30".to_owned()),
        event_date: date(2031, 3, 31),
        event_at: None,
        time_of_day: Some(CalendarEventTimeOfDay::PostMarket),
    }
}

fn normalize_checked_at(mut body: Value) -> Value {
    if let Some(events) = body["events"].as_array_mut() {
        for event in events {
            if let Some(references) = event.get_mut("references").and_then(Value::as_array_mut) {
                for reference in references {
                    reference["checked_at"] = json!("2030-01-01T00:00:00Z");
                }
            }
        }
    }
    body
}

#[backend_test_macros::database_test]
async fn sec_filing_references_are_available_for_targeted_and_expanded_us_earnings(
    db: gateway_postgres::DatabaseHandle,
) {
    let use_cases = build_use_cases_without_agent_task_client(db.clone());
    use_cases
        .us_stock_master()
        .sync(&FixedUsStockMasterSource)
        .await
        .expect("US stock master sync succeeds");

    let strategy_id = insert_test_strategy(&db, "sample-strategy").await;
    let target = gateway_postgres::entities::strategy_earnings_target::ActiveModel {
        strategy_id: Set(strategy_id),
        ref_kind: Set("stock".to_owned()),
        ref_id: Set("US:QZ7".to_owned()),
        created_at: Set(chrono::Utc::now().fixed_offset()),
    };
    target
        .insert(&db)
        .await
        .expect("strategy earnings target inserts");

    let repository = PostgresCalendarEventRepository::new(db.clone());
    let unit_of_work = PostgresUnitOfWork::new(db.clone());
    let transaction = unit_of_work.begin().await.expect("transaction begins");
    repository
        .upsert(
            &transaction,
            vec![
                us_earnings_event("QZ7", "架空銘柄 A"),
                us_earnings_event("QZ8", "架空銘柄 B"),
                us_earnings_event("QZ9", "架空銘柄 C"),
            ],
        )
        .await
        .expect("US earnings events insert");
    unit_of_work
        .commit(transaction)
        .await
        .expect("transaction commits");

    let stats = use_cases
        .sec_filing_references()
        .fetch_pending(&FixedSecFilingSource, date(2031, 4, 1))
        .await
        .expect("SEC filing references fetch succeeds");

    assert_eq!(
        stats,
        core_application::sec_filing_reference::SecFilingReferenceStats {
            events_checked: 2,
            companies_checked: 2,
            external_failures: 0,
            references_upserted: 2,
            budget_exhausted: false,
        },
    );

    let server = create_test_server(db).await;
    let calendar_response = server
        .get(&format!(
            "/api/calendar/events?from=2031-03-31&to=2031-03-31&strategy_id={strategy_id}"
        ))
        .await;
    let expanded_response = server
        .get(&format!(
            "/api/calendar/other-earnings?event_date=2031-03-31&country=US&strategy_id={strategy_id}"
        ))
        .await;

    assert_eq!(
        (
            calendar_response.status_code(),
            normalize_checked_at(calendar_response.json::<Value>()),
            expanded_response.status_code(),
            normalize_checked_at(expanded_response.json::<Value>()),
        ),
        (
            axum::http::StatusCode::OK,
            json!({
                "from": "2031-03-31",
                "to": "2031-03-31",
                "events": [
                    {
                        "kind": "event",
                        "source": "alpha_vantage",
                        "external_id": "US:QZ7:2031-03-30",
                        "category": "earnings",
                        "country": "US",
                        "title": "架空銘柄 A",
                        "stock_id": "US:QZ7",
                        "fiscal_period": "2031-03-30",
                        "fiscal_period_type": null,
                        "event_date": "2031-03-31",
                        "event_at": null,
                        "time_of_day": "post_market",
                        "references": [
                            {
                                "kind": "individual_document",
                                "source": "sec",
                                "external_id": "0900000001-99-000001",
                                "title": "SEC Form 10-Q",
                                "url": "https://example.invalid/Archives/edgar/data/900000001/090000000199000001/report.htm",
                                "target_period_start": null,
                                "target_period_end": "2031-03-30",
                                "target_period_type": "10-Q",
                                "publication_evidence": "SEC submissions record",
                                "publication_evidence_url": null,
                                "checked_at": "2030-01-01T00:00:00Z"
                            }
                        ],
                        "has_matching_earnings_summary": false
                    },
                    {
                        "kind": "other_earnings_summary",
                        "country": "US",
                        "event_date": "2031-03-31",
                        "count": 2
                    }
                ]
            }),
            axum::http::StatusCode::OK,
            json!({
                "events": [
                    {
                        "kind": "event",
                        "source": "alpha_vantage",
                        "external_id": "US:QZ8:2031-03-30",
                        "category": "earnings",
                        "country": "US",
                        "title": "架空銘柄 B",
                        "stock_id": "US:QZ8",
                        "fiscal_period": "2031-03-30",
                        "fiscal_period_type": null,
                        "event_date": "2031-03-31",
                        "event_at": null,
                        "time_of_day": "post_market",
                        "references": [
                            {
                                "kind": "individual_document",
                                "source": "sec",
                                "external_id": "0900000002-99-000001",
                                "title": "SEC Form 10-Q",
                                "url": "https://example.invalid/Archives/edgar/data/900000002/090000000299000001/report.htm",
                                "target_period_start": null,
                                "target_period_end": "2031-03-30",
                                "target_period_type": "10-Q",
                                "publication_evidence": "SEC submissions record",
                                "publication_evidence_url": null,
                                "checked_at": "2030-01-01T00:00:00Z"
                            }
                        ],
                        "has_matching_earnings_summary": false
                    },
                    {
                        "kind": "event",
                        "source": "alpha_vantage",
                        "external_id": "US:QZ9:2031-03-30",
                        "category": "earnings",
                        "country": "US",
                        "title": "架空銘柄 C",
                        "stock_id": "US:QZ9",
                        "fiscal_period": "2031-03-30",
                        "fiscal_period_type": null,
                        "event_date": "2031-03-31",
                        "event_at": null,
                        "time_of_day": "post_market",
                        "references": [],
                        "has_matching_earnings_summary": false
                    }
                ]
            }),
        ),
    );
}
