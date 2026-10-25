use chrono::NaiveDate;
use core_application::{calendar::repository::CalendarEventRepository, unit_of_work::UnitOfWork};
use core_domain::calendar_event::{
    CalendarEvent, CalendarEventCategory, CalendarEventReference, CalendarEventReferenceKind,
    CalendarEventTimeOfDay,
};
use gateway_postgres::{
    DatabaseHandle, PostgresCalendarEventRepository, PostgresUnitOfWork,
    entities::{financial_summary, strategy_earnings_target},
};
use sea_orm::{ActiveModelTrait, ActiveValue::NotSet, ActiveValue::Set};
use uuid::Uuid;

use crate::testing::{insert_test_group_membership, insert_test_stock, insert_test_strategy};

pub(crate) struct CalendarTestFixture {
    pub(crate) strategy_id: Uuid,
    pub(crate) from: NaiveDate,
    pub(crate) to: NaiveDate,
    pub(crate) event_date: NaiveDate,
}

impl CalendarTestFixture {
    pub(crate) async fn seed(db: &DatabaseHandle) -> Self {
        let strategy_id = insert_test_strategy(db, "sample-strategy").await;
        insert_test_stock(db, "0001", "サンプル銘柄 A").await;
        insert_test_stock(db, "0002", "サンプル銘柄 B").await;
        insert_test_stock(db, "0003", "サンプル銘柄 C").await;
        insert_test_group_membership(
            db,
            "0002",
            "sample-axis",
            "sample-group",
            "サンプル分類",
            None,
        )
        .await;
        for (ref_kind, ref_id) in [("stock", "0001"), ("group", "sample-axis/sample-group")] {
            strategy_earnings_target::ActiveModel {
                strategy_id: Set(strategy_id),
                ref_kind: Set(ref_kind.into()),
                ref_id: Set(ref_id.into()),
                created_at: NotSet,
            }
            .insert(db)
            .await
            .expect("insert earnings target");
        }

        let from = date(2031, 2, 10);
        let to = date(2031, 2, 16);
        let event_date = date(2031, 2, 11);
        let repository = PostgresCalendarEventRepository::new(db.clone());
        let unit_of_work = PostgresUnitOfWork::new(db.clone());
        let transaction = unit_of_work.begin().await.expect("begin transaction");
        repository
            .upsert(
                &transaction,
                vec![
                    event("00010:2Q:2031-03-31", "00010", "サンプル銘柄 A", event_date),
                    event("00020:2Q:2031-03-31", "00020", "サンプル銘柄 B", event_date),
                    event("00030:2Q:2031-03-31", "00030", "サンプル銘柄 C", event_date),
                ],
            )
            .await
            .expect("insert calendar events");
        repository
            .upsert_references(
                &transaction,
                ["00010:2Q:2031-03-31", "00030:2Q:2031-03-31"]
                    .into_iter()
                    .map(|external_id| CalendarEventReference {
                        event_source: "jquants".into(),
                        event_external_id: external_id.into(),
                        kind: CalendarEventReferenceKind::IndividualDocument,
                        source: "sample_source".into(),
                        external_id: format!("document-{external_id}"),
                        title: "サンプル決算資料".into(),
                        url: Some("https://example.invalid/sample-document".into()),
                        target_period_start: None,
                        target_period_end: Some(date(2031, 3, 31)),
                        target_period_type: Some("2Q".into()),
                        publication_evidence: Some("sample-release-record".into()),
                        publication_evidence_url: Some(
                            "https://example.invalid/sample-evidence".into(),
                        ),
                        checked_at: chrono::DateTime::from_timestamp(1_700_000_000, 0)
                            .expect("valid timestamp"),
                    })
                    .collect(),
            )
            .await
            .expect("insert calendar event references");
        unit_of_work
            .commit(transaction)
            .await
            .expect("commit transaction");
        for (code, disclosure_no, report_group_key, document_type, sales) in [
            (
                "00010",
                "101",
                "sample-actual-group-a",
                "2QFinancialStatements_SampleA",
                100.0,
            ),
            (
                "00030",
                "301",
                "sample-actual-group-c",
                "2QFinancialStatements_SampleB",
                300.0,
            ),
        ] {
            financial_summary::ActiveModel {
                code: Set(code.into()),
                disclosure_no: Set(disclosure_no.into()),
                disclosure_date: Set(event_date),
                report_group_key: Set(report_group_key.into()),
                document_type: Set(Some(document_type.into())),
                current_period_type: Set(Some("2Q".into())),
                current_period_end: Set(Some(date(2031, 3, 31))),
                sales: Set(Some(sales)),
                ..Default::default()
            }
            .insert(db)
            .await
            .expect("insert sample financial summary");
        }

        Self {
            strategy_id,
            from,
            to,
            event_date,
        }
    }
}

pub(crate) fn date(year: i32, month: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(year, month, day).expect("valid date")
}

fn event(external_id: &str, stock_id: &str, title: &str, event_date: NaiveDate) -> CalendarEvent {
    CalendarEvent {
        source: "jquants".into(),
        external_id: external_id.into(),
        category: CalendarEventCategory::Earnings,
        country: "JP".into(),
        title: title.into(),
        stock_id: Some(stock_id.into()),
        fiscal_period: Some("2031-03-31".into()),
        event_date,
        event_at: None,
        time_of_day: Some(CalendarEventTimeOfDay::PreMarket),
    }
}
