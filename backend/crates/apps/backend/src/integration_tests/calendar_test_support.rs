use chrono::NaiveDate;
use core_application::{calendar::repository::CalendarEventRepository, unit_of_work::UnitOfWork};
use core_domain::calendar_event::{CalendarEvent, CalendarEventCategory, CalendarEventTimeOfDay};
use gateway_postgres::{
    DatabaseHandle, PostgresCalendarEventRepository, PostgresUnitOfWork,
    entities::strategy_earnings_target,
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
                    event("tracked-direct", "00010", "サンプル銘柄 A", event_date),
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
