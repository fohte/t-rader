use std::{collections::HashMap, sync::Arc};

use async_trait::async_trait;
use chrono::{Datelike, Duration, NaiveDate, Utc};
use core_domain::calendar_event::{CalendarEvent, CalendarEventCategory};

use crate::{
    calendar::repository::{CalendarEventRepositoryError, SharedCalendarEventRepository},
    daily_bar_source::DateRange,
    strategy_scope::StrategyScope,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CalendarEventReadItem {
    Event(CalendarEvent),
    OtherEarningsSummary {
        country: String,
        event_date: NaiveDate,
        count: usize,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalendarEventReadResult {
    pub date_range: DateRange,
    pub events: Vec<CalendarEventReadItem>,
}

#[derive(Debug, thiserror::Error)]
pub enum CalendarEventTargetSourceError {
    #[error("{0}")]
    Failed(String),
}

#[async_trait]
pub trait CalendarEventTargetSource: Send + Sync {
    async fn list_stock_ids(
        &self,
        scope: StrategyScope,
    ) -> Result<Vec<String>, CalendarEventTargetSourceError>;
}

pub type SharedCalendarEventTargetSource = Arc<dyn CalendarEventTargetSource>;

#[derive(Debug, thiserror::Error)]
pub enum CalendarEventReadUseCaseError {
    #[error(transparent)]
    Repository(#[from] CalendarEventRepositoryError),
    #[error(transparent)]
    TargetSource(#[from] CalendarEventTargetSourceError),
    #[error("calendar event query range is invalid")]
    InvalidDateRange,
    #[error("calendar event summary index is invalid")]
    SummaryInvariant,
}

#[derive(Clone)]
pub struct CalendarEventReadUseCases {
    repository: SharedCalendarEventRepository,
    target_source: SharedCalendarEventTargetSource,
}

impl CalendarEventReadUseCases {
    pub fn new(
        repository: SharedCalendarEventRepository,
        target_source: SharedCalendarEventTargetSource,
    ) -> Self {
        Self {
            repository,
            target_source,
        }
    }

    pub async fn list_events(
        &self,
        from: Option<NaiveDate>,
        to: Option<NaiveDate>,
        strategy_scope: Option<StrategyScope>,
    ) -> Result<CalendarEventReadResult, CalendarEventReadUseCaseError> {
        let today = (Utc::now().naive_utc() + Duration::hours(9)).date();
        let date_range = resolve_date_range(from, to, today);
        if date_range.from > date_range.to {
            return Err(CalendarEventReadUseCaseError::InvalidDateRange);
        }

        let tracked_stock_ids = self.tracked_stock_ids(strategy_scope).await?;

        let events = self.repository.list_events(&date_range).await?;
        let mut items = Vec::with_capacity(events.len());
        let mut summary_indices = HashMap::new();
        for event in events {
            if event.category != CalendarEventCategory::Earnings
                || event
                    .stock_id
                    .as_ref()
                    .is_some_and(|stock_id| tracked_stock_ids.contains(stock_id))
            {
                items.push(CalendarEventReadItem::Event(event));
                continue;
            }

            let key = (event.country.clone(), event.event_date);
            if let Some(index) = summary_indices.get(&key) {
                match items.get_mut(*index) {
                    Some(CalendarEventReadItem::OtherEarningsSummary { count, .. }) => *count += 1,
                    _ => return Err(CalendarEventReadUseCaseError::SummaryInvariant),
                }
            } else {
                summary_indices.insert(key.clone(), items.len());
                items.push(CalendarEventReadItem::OtherEarningsSummary {
                    country: key.0,
                    event_date: key.1,
                    count: 1,
                });
            }
        }

        Ok(CalendarEventReadResult {
            date_range,
            events: items,
        })
    }

    pub async fn list_other_earnings(
        &self,
        event_date: NaiveDate,
        country: &str,
        strategy_scope: Option<StrategyScope>,
    ) -> Result<Vec<CalendarEvent>, CalendarEventReadUseCaseError> {
        let tracked_stock_ids = self.tracked_stock_ids(strategy_scope).await?;
        let date_range = DateRange {
            from: event_date,
            to: event_date,
        };

        Ok(self
            .repository
            .list_events(&date_range)
            .await?
            .into_iter()
            .filter(|event| {
                event.category == CalendarEventCategory::Earnings
                    && event.country == country
                    && !event
                        .stock_id
                        .as_ref()
                        .is_some_and(|stock_id| tracked_stock_ids.contains(stock_id))
            })
            .collect())
    }

    async fn tracked_stock_ids(
        &self,
        strategy_scope: Option<StrategyScope>,
    ) -> Result<std::collections::HashSet<String>, CalendarEventReadUseCaseError> {
        Ok(match strategy_scope {
            Some(scope) => self
                .target_source
                .list_stock_ids(scope)
                .await?
                .into_iter()
                .collect::<std::collections::HashSet<_>>(),
            None => std::collections::HashSet::new(),
        })
    }
}

fn resolve_date_range(
    from: Option<NaiveDate>,
    to: Option<NaiveDate>,
    today: NaiveDate,
) -> DateRange {
    let monday = today - Duration::days(i64::from(today.weekday().num_days_from_monday()));
    DateRange {
        from: from.unwrap_or(monday),
        to: to.unwrap_or(monday + Duration::days(6)),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use async_trait::async_trait;
    use chrono::{DateTime, NaiveDate, Utc};
    use core_domain::calendar_event::{
        CalendarEvent, CalendarEventCategory, CalendarEventTimeOfDay,
    };
    use uuid::Uuid;

    use crate::{
        calendar::{
            read_use_cases::{
                CalendarEventReadItem, CalendarEventReadResult, CalendarEventReadUseCases,
                CalendarEventTargetSource, CalendarEventTargetSourceError,
                SharedCalendarEventTargetSource, resolve_date_range,
            },
            repository::{
                CalendarEventRepository, CalendarEventRepositoryError,
                SharedCalendarEventRepository,
            },
        },
        daily_bar_source::DateRange,
        strategy_scope::{StrategyScope, StrategyScopeSource, StrategyScopeSourceError},
        unit_of_work::UnitOfWorkTransaction,
    };

    #[derive(Default)]
    struct FakeCalendarEventRepository {
        events: Vec<CalendarEvent>,
    }

    #[async_trait]
    impl CalendarEventRepository for FakeCalendarEventRepository {
        async fn list_events(
            &self,
            date_range: &DateRange,
        ) -> Result<Vec<CalendarEvent>, CalendarEventRepositoryError> {
            Ok(self
                .events
                .iter()
                .filter(|event| {
                    event.event_date >= date_range.from && event.event_date <= date_range.to
                })
                .cloned()
                .collect())
        }

        async fn upsert(
            &self,
            _transaction: &UnitOfWorkTransaction,
            _events: Vec<CalendarEvent>,
        ) -> Result<usize, CalendarEventRepositoryError> {
            Ok(0)
        }

        async fn delete_missing_future_events(
            &self,
            _transaction: &UnitOfWorkTransaction,
            _source: &str,
            _date_range: &DateRange,
            _today: NaiveDate,
            _external_ids: Vec<String>,
        ) -> Result<u64, CalendarEventRepositoryError> {
            Ok(0)
        }
    }

    struct FakeTargetSource {
        stock_ids: Vec<String>,
        calls: Mutex<usize>,
    }

    #[async_trait]
    impl CalendarEventTargetSource for FakeTargetSource {
        async fn list_stock_ids(
            &self,
            _scope: StrategyScope,
        ) -> Result<Vec<String>, CalendarEventTargetSourceError> {
            *self.calls.lock().expect("calls lock") += 1;
            Ok(self.stock_ids.clone())
        }
    }

    struct FakeStrategyScopeSource;

    #[async_trait]
    impl StrategyScopeSource for FakeStrategyScopeSource {
        async fn existing_ids(
            &self,
            ids: &[Uuid],
        ) -> Result<std::collections::HashSet<Uuid>, StrategyScopeSourceError> {
            Ok(ids.iter().copied().collect())
        }
    }

    fn date(year: i32, month: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(year, month, day).expect("valid date")
    }

    fn event(
        external_id: &str,
        category: CalendarEventCategory,
        country: &str,
        title: &str,
        stock_id: Option<&str>,
        event_date: NaiveDate,
    ) -> CalendarEvent {
        CalendarEvent {
            source: "sample_source".into(),
            external_id: external_id.into(),
            category,
            country: country.into(),
            title: title.into(),
            stock_id: stock_id.map(str::to_owned),
            fiscal_period: Some("2031-03-31".into()),
            event_date,
            event_at: Some(DateTime::from_naive_utc_and_offset(
                event_date.and_hms_opt(9, 0, 0).expect("valid time"),
                Utc,
            )),
            time_of_day: Some(CalendarEventTimeOfDay::PreMarket),
        }
    }

    async fn strategy_scope() -> StrategyScope {
        StrategyScope::verify(Uuid::from_u128(1), &FakeStrategyScopeSource)
            .await
            .expect("known strategy")
    }

    fn use_cases(
        events: Vec<CalendarEvent>,
        stock_ids: Vec<String>,
    ) -> (CalendarEventReadUseCases, Arc<FakeTargetSource>) {
        let targets = Arc::new(FakeTargetSource {
            stock_ids,
            calls: Mutex::new(0),
        });
        let use_cases = CalendarEventReadUseCases::new(
            Arc::new(FakeCalendarEventRepository { events }) as SharedCalendarEventRepository,
            targets.clone() as SharedCalendarEventTargetSource,
        );
        (use_cases, targets)
    }

    #[tokio::test]
    async fn list_events_keeps_tracked_stocks_and_summarizes_other_earnings_by_country_and_date() {
        let date_range = DateRange {
            from: date(2031, 2, 10),
            to: date(2031, 2, 16),
        };
        let events = vec![
            event(
                "indicator",
                CalendarEventCategory::Indicator,
                "JP",
                "サンプル指標",
                None,
                date(2031, 2, 11),
            ),
            event(
                "tracked-direct",
                CalendarEventCategory::Earnings,
                "JP",
                "サンプル銘柄 A",
                Some("0001"),
                date(2031, 2, 11),
            ),
            event(
                "tracked-group",
                CalendarEventCategory::Earnings,
                "US",
                "サンプル銘柄 B",
                Some("US:SAMPLE-B"),
                date(2031, 2, 11),
            ),
            event(
                "other-jp-a",
                CalendarEventCategory::Earnings,
                "JP",
                "サンプル銘柄 C",
                Some("0003"),
                date(2031, 2, 11),
            ),
            event(
                "other-jp-b",
                CalendarEventCategory::Earnings,
                "JP",
                "サンプル銘柄 D",
                Some("0004"),
                date(2031, 2, 11),
            ),
            event(
                "other-us",
                CalendarEventCategory::Earnings,
                "US",
                "サンプル銘柄 E",
                Some("US:SAMPLE-E"),
                date(2031, 2, 11),
            ),
            event(
                "central-bank",
                CalendarEventCategory::CentralBank,
                "EU",
                "サンプル中銀イベント",
                None,
                date(2031, 2, 12),
            ),
            event(
                "outside-range",
                CalendarEventCategory::Earnings,
                "JP",
                "範囲外の銘柄",
                Some("0001"),
                date(2031, 2, 17),
            ),
        ];
        let (use_cases, _) = use_cases(events, vec!["0001".into(), "US:SAMPLE-B".into()]);

        let result = use_cases
            .list_events(
                Some(date_range.from),
                Some(date_range.to),
                Some(strategy_scope().await),
            )
            .await
            .expect("list calendar events");

        assert_eq!(
            result,
            CalendarEventReadResult {
                date_range,
                events: vec![
                    CalendarEventReadItem::Event(event(
                        "indicator",
                        CalendarEventCategory::Indicator,
                        "JP",
                        "サンプル指標",
                        None,
                        date(2031, 2, 11),
                    )),
                    CalendarEventReadItem::Event(event(
                        "tracked-direct",
                        CalendarEventCategory::Earnings,
                        "JP",
                        "サンプル銘柄 A",
                        Some("0001"),
                        date(2031, 2, 11),
                    )),
                    CalendarEventReadItem::Event(event(
                        "tracked-group",
                        CalendarEventCategory::Earnings,
                        "US",
                        "サンプル銘柄 B",
                        Some("US:SAMPLE-B"),
                        date(2031, 2, 11),
                    )),
                    CalendarEventReadItem::OtherEarningsSummary {
                        country: "JP".into(),
                        event_date: date(2031, 2, 11),
                        count: 2,
                    },
                    CalendarEventReadItem::OtherEarningsSummary {
                        country: "US".into(),
                        event_date: date(2031, 2, 11),
                        count: 1,
                    },
                    CalendarEventReadItem::Event(event(
                        "central-bank",
                        CalendarEventCategory::CentralBank,
                        "EU",
                        "サンプル中銀イベント",
                        None,
                        date(2031, 2, 12),
                    )),
                ],
            }
        );
    }

    #[tokio::test]
    async fn list_events_summarizes_all_earnings_when_strategy_is_omitted() {
        let events = vec![
            event(
                "first",
                CalendarEventCategory::Earnings,
                "JP",
                "サンプル銘柄 A",
                Some("0001"),
                date(2031, 2, 11),
            ),
            event(
                "second",
                CalendarEventCategory::Earnings,
                "JP",
                "サンプル銘柄 B",
                Some("0002"),
                date(2031, 2, 11),
            ),
        ];
        let (use_cases, target_source) = use_cases(events, vec!["0001".into()]);

        let result = use_cases
            .list_events(Some(date(2031, 2, 10)), Some(date(2031, 2, 16)), None)
            .await
            .expect("list calendar events");

        assert_eq!(
            (result, *target_source.calls.lock().expect("calls lock")),
            (
                CalendarEventReadResult {
                    date_range: DateRange {
                        from: date(2031, 2, 10),
                        to: date(2031, 2, 16),
                    },
                    events: vec![CalendarEventReadItem::OtherEarningsSummary {
                        country: "JP".into(),
                        event_date: date(2031, 2, 11),
                        count: 2,
                    }],
                },
                0
            )
        );
    }

    #[tokio::test]
    async fn list_other_earnings_returns_untracked_earnings_for_the_requested_country_and_date() {
        let target_date = date(2031, 2, 11);
        let events = vec![
            event(
                "tracked",
                CalendarEventCategory::Earnings,
                "JP",
                "サンプル銘柄 A",
                Some("0001"),
                target_date,
            ),
            event(
                "other-jp",
                CalendarEventCategory::Earnings,
                "JP",
                "サンプル銘柄 B",
                Some("0002"),
                target_date,
            ),
            event(
                "other-us",
                CalendarEventCategory::Earnings,
                "US",
                "サンプル銘柄 C",
                Some("US:SAMPLE-C"),
                target_date,
            ),
            event(
                "other-date",
                CalendarEventCategory::Earnings,
                "JP",
                "別日の銘柄",
                Some("0003"),
                date(2031, 2, 12),
            ),
            event(
                "indicator",
                CalendarEventCategory::Indicator,
                "JP",
                "サンプル指標",
                None,
                target_date,
            ),
        ];
        let (use_cases, _) = use_cases(events, vec!["0001".into()]);

        let result = use_cases
            .list_other_earnings(target_date, "JP", Some(strategy_scope().await))
            .await
            .expect("list other earnings");

        assert_eq!(
            result,
            vec![event(
                "other-jp",
                CalendarEventCategory::Earnings,
                "JP",
                "サンプル銘柄 B",
                Some("0002"),
                target_date,
            )]
        );
    }

    #[tokio::test]
    async fn list_other_earnings_returns_all_matching_earnings_when_strategy_is_omitted() {
        let target_date = date(2031, 2, 11);
        let events = vec![
            event(
                "first",
                CalendarEventCategory::Earnings,
                "JP",
                "サンプル銘柄 A",
                Some("0001"),
                target_date,
            ),
            event(
                "second",
                CalendarEventCategory::Earnings,
                "JP",
                "サンプル銘柄 B",
                Some("0002"),
                target_date,
            ),
        ];
        let (use_cases, target_source) = use_cases(events, vec!["0001".into()]);

        let result = use_cases
            .list_other_earnings(target_date, "JP", None)
            .await
            .expect("list other earnings");

        assert_eq!(
            (result, *target_source.calls.lock().expect("calls lock")),
            (
                vec![
                    event(
                        "first",
                        CalendarEventCategory::Earnings,
                        "JP",
                        "サンプル銘柄 A",
                        Some("0001"),
                        target_date,
                    ),
                    event(
                        "second",
                        CalendarEventCategory::Earnings,
                        "JP",
                        "サンプル銘柄 B",
                        Some("0002"),
                        target_date,
                    ),
                ],
                0,
            )
        );
    }

    #[rstest::rstest]
    #[case::monday(date(2031, 2, 10), date(2031, 2, 10), date(2031, 2, 16))]
    #[case::sunday(date(2031, 2, 16), date(2031, 2, 10), date(2031, 2, 16))]
    fn resolve_date_range_defaults_to_jst_calendar_week(
        #[case] today: NaiveDate,
        #[case] expected_from: NaiveDate,
        #[case] expected_to: NaiveDate,
    ) {
        assert_eq!(
            resolve_date_range(None, None, today),
            DateRange {
                from: expected_from,
                to: expected_to,
            }
        );
    }
}
