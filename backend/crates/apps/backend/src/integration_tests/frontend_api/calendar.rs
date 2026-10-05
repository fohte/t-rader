use chrono::NaiveDate;
use core_application::{calendar::repository::CalendarEventRepository, unit_of_work::UnitOfWork};
use core_domain::calendar_event::{CalendarEvent, CalendarEventCategory, CalendarEventTimeOfDay};
use gateway_postgres::{
    PostgresCalendarEventRepository, PostgresUnitOfWork, entities::strategy_earnings_target,
};
use sea_orm::{ActiveModelTrait, ActiveValue::NotSet, ActiveValue::Set};
use serde_json::json;

use crate::testing::{
    create_test_server, insert_test_group_membership, insert_test_stock, insert_test_strategy,
};

#[backend_test_macros::database_test]
async fn calendar_endpoint_keeps_targeted_earnings_and_summarizes_the_rest(
    db: gateway_postgres::DatabaseHandle,
) {
    let strategy_id = insert_test_strategy(&db, "sample-strategy").await;
    insert_test_stock(&db, "0001", "サンプル銘柄 A").await;
    insert_test_stock(&db, "0002", "サンプル銘柄 B").await;
    insert_test_stock(&db, "0003", "サンプル銘柄 C").await;
    insert_test_group_membership(
        &db,
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
        .insert(&db)
        .await
        .expect("insert earnings target");
    }

    let event_date = date(2031, 2, 11);
    let repository = PostgresCalendarEventRepository::new(db.clone());
    let unit_of_work = PostgresUnitOfWork::new(db.clone());
    let transaction = unit_of_work.begin().await.expect("begin transaction");
    repository
        .upsert(
            &transaction,
            vec![
                event("tracked-direct", "0001", "サンプル銘柄 A", event_date),
                event("tracked-group", "0002", "サンプル銘柄 B", event_date),
                event("untracked", "0003", "サンプル銘柄 C", event_date),
            ],
        )
        .await
        .expect("insert calendar events");
    unit_of_work
        .commit(transaction)
        .await
        .expect("commit transaction");

    let server = create_test_server(db).await;
    let strategy_id = strategy_id.to_string();
    let response = server
        .get(&format!(
            "/api/calendar/events?from=2031-02-10&to=2031-02-16&strategy_id={strategy_id}"
        ))
        .await;

    assert_eq!(
        (response.status_code(), response.json::<serde_json::Value>(),),
        (
            axum::http::StatusCode::OK,
            json!({
                "from": "2031-02-10",
                "to": "2031-02-16",
                "events": [
                    {
                        "kind": "event",
                        "source": "sample_source",
                        "external_id": "tracked-direct",
                        "category": "earnings",
                        "country": "JP",
                        "title": "サンプル銘柄 A",
                        "stock_id": "0001",
                        "fiscal_period": "2031-03-31",
                        "event_date": "2031-02-11",
                        "event_at": null,
                        "time_of_day": "pre_market",
                    },
                    {
                        "kind": "event",
                        "source": "sample_source",
                        "external_id": "tracked-group",
                        "category": "earnings",
                        "country": "JP",
                        "title": "サンプル銘柄 B",
                        "stock_id": "0002",
                        "fiscal_period": "2031-03-31",
                        "event_date": "2031-02-11",
                        "event_at": null,
                        "time_of_day": "pre_market",
                    },
                    {
                        "kind": "other_earnings_summary",
                        "country": "JP",
                        "event_date": "2031-02-11",
                        "count": 1,
                    },
                ],
            }),
        ),
    );
}

fn date(year: i32, month: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(year, month, day).expect("valid date")
}

fn event(external_id: &str, stock_id: &str, title: &str, event_date: NaiveDate) -> CalendarEvent {
    CalendarEvent {
        source: "sample_source".into(),
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
