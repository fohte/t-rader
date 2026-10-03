use std::collections::BTreeMap;

use async_trait::async_trait;
use chrono::NaiveDate;
use core_application::{
    calendar::source::{CalendarEventBatch, CalendarEventSource, CalendarEventSourceError},
    daily_bar_source::DateRange,
};
use reqwest::Url;

use crate::{
    SOURCE_NAME,
    month::{first_day_of_month, last_day_of_month, next_month},
    parser::parse_release_calendar,
};

const RELEASE_CALENDAR_URL: &str = "https://www.e-stat.go.jp/release-calendar";
const HTTP_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(20);
const MAX_LOOKAHEAD_MONTHS: u32 = 12;

pub struct EStatCalendarEventSource {
    http: reqwest::Client,
    base_url: Url,
}

impl EStatCalendarEventSource {
    pub fn new() -> Result<Self, CalendarEventSourceError> {
        let base_url = Url::parse(RELEASE_CALENDAR_URL).map_err(|error| {
            CalendarEventSourceError::Failed(format!(
                "invalid e-Stat release calendar URL: {error}"
            ))
        })?;
        Self::from_base_url(base_url)
    }

    fn from_base_url(base_url: Url) -> Result<Self, CalendarEventSourceError> {
        let http = reqwest::Client::builder()
            .timeout(HTTP_TIMEOUT)
            .user_agent("t-rader calendar importer")
            .build()
            .map_err(|error| {
                CalendarEventSourceError::Failed(format!(
                    "failed to initialize e-Stat HTTP client: {error}"
                ))
            })?;

        Ok(Self { http, base_url })
    }

    async fn fetch_month(
        &self,
        month: NaiveDate,
    ) -> Result<crate::parser::ParsedCalendarMonth, CalendarEventSourceError> {
        let month_param = month.format("%Y%m").to_string();
        let mut url = self.base_url.clone();
        url.query_pairs_mut()
            .append_pair("page_month", &month_param);
        let response = self.http.get(url).send().await.map_err(|error| {
            CalendarEventSourceError::Failed(format!(
                "failed to fetch e-Stat release calendar for {month_param}: {error}"
            ))
        })?;
        let status = response.status();
        if !status.is_success() {
            return Err(CalendarEventSourceError::Failed(format!(
                "e-Stat release calendar returned HTTP {status} for {month_param}"
            )));
        }
        let html = response.text().await.map_err(|error| {
            CalendarEventSourceError::Failed(format!(
                "failed to read e-Stat release calendar for {month_param}: {error}"
            ))
        })?;

        parse_release_calendar(&html, month).map_err(CalendarEventSourceError::Failed)
    }
}

#[async_trait]
impl CalendarEventSource for EStatCalendarEventSource {
    fn source(&self) -> &str {
        SOURCE_NAME
    }

    async fn fetch_calendar_events(
        &self,
        today: NaiveDate,
    ) -> Result<CalendarEventBatch, CalendarEventSourceError> {
        let mut month = first_day_of_month(today).ok_or_else(|| {
            CalendarEventSourceError::Failed(format!("invalid month in requested date: {today}"))
        })?;
        let mut range_from = today;
        let mut events = BTreeMap::new();
        let mut range_to = None;

        for lookahead_index in 0..MAX_LOOKAHEAD_MONTHS {
            let parsed_month = self.fetch_month(month).await?;
            // 不確定期間の前後にある確定予定も upsert し、欠落予定の削除範囲だけを安全な連続期間に絞る。
            events.extend(
                parsed_month
                    .events
                    .into_iter()
                    .filter(|event| event.event_date >= today)
                    .map(|event| (event.external_id.clone(), event)),
            );

            for period in &parsed_month.approximate_periods {
                if period.to < range_from {
                    continue;
                }
                if period.from <= range_from {
                    range_from = period.to.succ_opt().ok_or_else(|| {
                        CalendarEventSourceError::Failed(
                            "e-Stat release calendar has no complete future date range".to_owned(),
                        )
                    })?;
                    continue;
                }

                let safe_to = period.from.pred_opt().ok_or_else(|| {
                    CalendarEventSourceError::Failed(
                        "e-Stat release calendar has no complete future date range".to_owned(),
                    )
                })?;
                return Ok(CalendarEventBatch {
                    date_range: DateRange {
                        from: range_from,
                        to: safe_to,
                    },
                    events: events.into_values().collect(),
                });
            }

            let month_end = last_day_of_month(month).ok_or_else(|| {
                CalendarEventSourceError::Failed(format!("invalid end of month: {month}"))
            })?;
            if range_from <= month_end {
                range_to = Some(month_end);
            }

            if lookahead_index + 1 < MAX_LOOKAHEAD_MONTHS {
                month = next_month(month).ok_or_else(|| {
                    CalendarEventSourceError::Failed(format!("month is out of range: {month}"))
                })?;
            }
        }

        let to = range_to.filter(|to| *to >= range_from).ok_or_else(|| {
            CalendarEventSourceError::Failed(
                "e-Stat release calendar returned no complete date range".to_owned(),
            )
        })?;
        Ok(CalendarEventBatch {
            date_range: DateRange {
                from: range_from,
                to,
            },
            events: events.into_values().collect(),
        })
    }
}

#[cfg(test)]
mod tests {
    use chrono::{DateTime, NaiveDate, Utc};
    use core_application::{
        calendar::source::{CalendarEventBatch, CalendarEventSource},
        daily_bar_source::DateRange,
    };
    use core_domain::calendar_event::{CalendarEvent, CalendarEventCategory};
    use indoc::indoc;
    use reqwest::Url;
    use wiremock::{
        Mock, MockServer, ResponseTemplate,
        matchers::{method, path, query_param},
    };

    use super::EStatCalendarEventSource;
    use crate::{SOURCE_NAME, parser::parse_release_calendar};

    #[tokio::test]
    async fn limits_deletion_range_before_a_later_approximate_release() {
        let server = MockServer::start().await;
        mount_month(
            &server,
            "202610",
            include_str!("../tests/fixtures/release-calendar-202610.html"),
        )
        .await;
        mount_month(
            &server,
            "202611",
            &html_with_rows(&[
                approximate_row("2026-11月中旬", "00350300", "202611110000", "普通貿易統計"),
                exact_row("2026-11-25 08:30", "00200531", "202611250830", "労働力調査"),
            ]),
        )
        .await;
        let source = source_for(&server);
        let today = date(2026, 10, 1);
        let mut events = parse_release_calendar(
            include_str!("../tests/fixtures/release-calendar-202610.html"),
            today,
        )
        .expect("October fixture parses")
        .events;
        events.push(event(
            "00200531",
            "労働力調査",
            "202611250830",
            date(2026, 11, 25),
            Some(utc_datetime("2026-11-24T23:30:00Z")),
        ));
        sort_events(&mut events);

        assert_eq!(
            source.fetch_calendar_events(today).await,
            Ok(CalendarEventBatch {
                date_range: DateRange {
                    from: date(2026, 10, 1),
                    to: date(2026, 11, 10),
                },
                events,
            }),
        );
    }

    #[tokio::test]
    async fn skips_an_overlapping_approximate_period_and_keeps_known_events() {
        let server = MockServer::start().await;
        mount_month(
            &server,
            "202703",
            include_str!("../tests/fixtures/release-calendar-202703.html"),
        )
        .await;
        mount_month(
            &server,
            "202704",
            &html_with_rows(&[exact_row(
                "2027-04-15 08:30",
                "00200531",
                "202704150830",
                "労働力調査",
            )]),
        )
        .await;
        mount_month(
            &server,
            "202705",
            &html_with_rows(&[
                approximate_row("2027-05月中旬", "00350300", "202705110000", "普通貿易統計"),
                exact_row(
                    "2027-05-25 08:50",
                    "00100401",
                    "202705250850",
                    "機械受注統計調査",
                ),
            ]),
        )
        .await;
        let source = source_for(&server);
        let today = date(2027, 3, 1);
        let mut events = parse_release_calendar(
            include_str!("../tests/fixtures/release-calendar-202703.html"),
            today,
        )
        .expect("March fixture parses")
        .events;
        events.extend([
            event(
                "00200531",
                "労働力調査",
                "202704150830",
                date(2027, 4, 15),
                Some(utc_datetime("2027-04-14T23:30:00Z")),
            ),
            event(
                "00100401",
                "機械受注統計調査",
                "202705250850",
                date(2027, 5, 25),
                Some(utc_datetime("2027-05-24T23:50:00Z")),
            ),
        ]);
        sort_events(&mut events);

        assert_eq!(
            source.fetch_calendar_events(today).await,
            Ok(CalendarEventBatch {
                date_range: DateRange {
                    from: date(2027, 4, 1),
                    to: date(2027, 5, 10),
                },
                events,
            }),
        );
    }

    async fn mount_month(server: &MockServer, month: &str, html: &str) {
        Mock::given(method("GET"))
            .and(path("/release-calendar"))
            .and(query_param("page_month", month))
            .respond_with(ResponseTemplate::new(200).set_body_string(html))
            .mount(server)
            .await;
    }

    fn source_for(server: &MockServer) -> EStatCalendarEventSource {
        let base_url = Url::parse(&format!("{}/release-calendar", server.uri()))
            .expect("valid mock server URL");
        EStatCalendarEventSource::from_base_url(base_url).expect("source initializes")
    }

    fn html_with_rows(rows: &[String]) -> String {
        format!("<ul class=\"stat-list-body\">{}</ul>", rows.join("\n"))
    }

    fn exact_row(release_date: &str, stat_code: &str, publication_id: &str, title: &str) -> String {
        format!(
            indoc! {r#"
                <li class="stat-list-row">
                  <span class="stat-announce-keisaiday">{release_date}</span>
                  <span class="stat-announce-comment" data-toukei_cd="{stat_code}" data-kensakuKouhyou_date="{publication_id}">
                    <a>{title}</a>
                  </span>
                </li>
            "#},
            release_date = release_date,
            stat_code = stat_code,
            publication_id = publication_id,
            title = title,
        )
    }

    fn approximate_row(
        release_date: &str,
        stat_code: &str,
        publication_id: &str,
        title: &str,
    ) -> String {
        format!(
            indoc! {r#"
                <li class="stat-list-row">
                  <span class="stat-announce-keisaiday">{release_date}</span>
                  <span class="stat-announce-comment" data-toukei_cd="{stat_code}" data-kensakuKouhyou_date="{publication_id}">
                    <a>{title}</a>
                  </span>
                </li>
            "#},
            release_date = release_date,
            stat_code = stat_code,
            publication_id = publication_id,
            title = title,
        )
    }

    fn event(
        stat_code: &str,
        title: &str,
        publication_id: &str,
        event_date: NaiveDate,
        event_at: Option<DateTime<Utc>>,
    ) -> CalendarEvent {
        CalendarEvent {
            source: SOURCE_NAME.to_owned(),
            external_id: format!("{stat_code}:{publication_id}"),
            category: CalendarEventCategory::Indicator,
            country: "JP".to_owned(),
            title: title.to_owned(),
            stock_id: None,
            fiscal_period: None,
            event_date,
            event_at,
            time_of_day: None,
        }
    }

    fn date(year: i32, month: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(year, month, day).expect("valid date")
    }

    fn utc_datetime(value: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(value)
            .expect("valid RFC 3339 datetime")
            .with_timezone(&Utc)
    }

    fn sort_events(events: &mut [CalendarEvent]) {
        events.sort_by(|left, right| left.external_id.cmp(&right.external_id));
    }
}
