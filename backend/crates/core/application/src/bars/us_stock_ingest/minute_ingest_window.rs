use chrono::{
    DateTime, Datelike, Duration, NaiveDate, NaiveTime, TimeZone, Timelike, Utc, Weekday,
};
use chrono_tz::America::New_York;

use super::super::US_STOCK_SIP_DELAY_MINUTES;

const INGEST_WINDOW_START_MINUTE: u32 = 4 * 60 + 20;
const INGEST_WINDOW_END_MINUTE: u32 = 20 * 60 + 20;

pub(super) fn is_us_stock_minute_ingest_window(now: DateTime<Utc>) -> bool {
    let eastern = now.with_timezone(&New_York);
    let minute_of_day = eastern.hour() * 60 + eastern.minute();

    is_weekday(eastern.weekday())
        && (INGEST_WINDOW_START_MINUTE..=INGEST_WINDOW_END_MINUTE).contains(&minute_of_day)
}

pub(super) fn most_recent_us_stock_minute_ingest_cutoff(
    now: DateTime<Utc>,
) -> Option<DateTime<Utc>> {
    let eastern = now.with_timezone(&New_York);
    let minute_of_day = eastern.hour() * 60 + eastern.minute();
    let date = if is_weekday(eastern.weekday()) && minute_of_day > INGEST_WINDOW_END_MINUTE {
        eastern.date_naive()
    } else {
        previous_weekday(eastern.date_naive())?
    };
    let cutoff_minute = i64::from(INGEST_WINDOW_END_MINUTE) - US_STOCK_SIP_DELAY_MINUTES;
    let local_cutoff = date.and_time(NaiveTime::MIN + Duration::minutes(cutoff_minute));

    New_York
        .from_local_datetime(&local_cutoff)
        .single()
        .map(|cutoff| cutoff.with_timezone(&Utc))
}

pub(super) fn should_ingest_us_stock_minute_bars(
    now: DateTime<Utc>,
    completed_through: Option<DateTime<Utc>>,
) -> bool {
    if is_us_stock_minute_ingest_window(now) {
        return true;
    }

    let Some(required_cutoff) = most_recent_us_stock_minute_ingest_cutoff(now) else {
        return true;
    };
    completed_through.is_none_or(|completed_through| completed_through < required_cutoff)
}

pub(super) fn all_us_stock_minute_targets_complete_for_closed_window(
    now: DateTime<Utc>,
    completed_through_values: impl IntoIterator<Item = Option<DateTime<Utc>>>,
) -> bool {
    let mut has_targets = false;
    for completed_through in completed_through_values {
        has_targets = true;
        if should_ingest_us_stock_minute_bars(now, completed_through) {
            return false;
        }
    }
    has_targets
}

fn previous_weekday(mut date: NaiveDate) -> Option<NaiveDate> {
    loop {
        date = date.pred_opt()?;
        if is_weekday(date.weekday()) {
            return Some(date);
        }
    }
}

fn is_weekday(weekday: Weekday) -> bool {
    !matches!(weekday, Weekday::Sat | Weekday::Sun)
}

#[cfg(test)]
mod tests {
    use chrono::{DateTime, Utc};
    use rstest::rstest;

    use super::{
        all_us_stock_minute_targets_complete_for_closed_window, is_us_stock_minute_ingest_window,
        most_recent_us_stock_minute_ingest_cutoff, should_ingest_us_stock_minute_bars,
    };

    fn date(value: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(value)
            .expect("valid fixture timestamp")
            .with_timezone(&Utc)
    }

    #[rstest]
    #[case::before_winter_open("2040-01-02T09:19:00Z", false)]
    #[case::at_winter_open("2040-01-02T09:20:00Z", true)]
    #[case::at_winter_close("2040-01-03T01:20:59Z", true)]
    #[case::after_winter_close("2040-01-03T01:21:00Z", false)]
    #[case::at_summer_open("2040-07-02T08:20:00Z", true)]
    #[case::at_summer_close("2040-07-03T00:20:59Z", true)]
    #[case::after_summer_close("2040-07-03T00:21:00Z", false)]
    #[case::saturday("2040-01-07T18:00:00Z", false)]
    #[case::sunday("2040-01-08T18:00:00Z", false)]
    fn market_window_uses_eastern_weekdays_and_follows_daylight_saving_time(
        #[case] now: &str,
        #[case] expected: bool,
    ) {
        assert_eq!(is_us_stock_minute_ingest_window(date(now)), expected);
    }

    #[rstest]
    #[case::all_targets_complete(
        "2040-01-08T18:00:00Z",
        vec![Some("2040-01-07T01:04:00Z"), Some("2040-01-07T01:05:00Z")],
        true
    )]
    #[case::target_is_incomplete(
        "2040-01-08T18:00:00Z",
        vec![Some("2040-01-07T01:04:00Z"), Some("2040-01-07T01:03:00Z")],
        false
    )]
    #[case::target_has_no_progress("2040-01-08T18:00:00Z", vec![None], false)]
    #[case::no_targets("2040-01-08T18:00:00Z", Vec::new(), false)]
    #[case::open_window("2040-01-02T09:20:00Z", vec![Some("2040-01-07T01:04:00Z")], false)]
    fn skips_only_when_every_minute_target_is_complete_in_a_closed_window(
        #[case] now: &str,
        #[case] completed_through: Vec<Option<&str>>,
        #[case] expected: bool,
    ) {
        let completed_through = completed_through.into_iter().map(|value| value.map(date));

        assert_eq!(
            all_us_stock_minute_targets_complete_for_closed_window(date(now), completed_through),
            expected,
        );
    }

    #[rstest]
    #[case::before_open_uses_previous_weekday("2040-01-03T09:19:00Z", "2040-01-03T01:04:00Z")]
    #[case::after_final_run_uses_same_day("2040-01-03T01:21:00Z", "2040-01-03T01:04:00Z")]
    #[case::saturday_uses_friday("2040-01-07T18:00:00Z", "2040-01-07T01:04:00Z")]
    #[case::sunday_uses_friday("2040-01-08T18:00:00Z", "2040-01-07T01:04:00Z")]
    #[case::summer_time_uses_daylight_offset("2040-07-03T00:21:00Z", "2040-07-03T00:04:00Z")]
    fn closed_window_cutoff_includes_the_final_run_delay(
        #[case] now: &str,
        #[case] expected: &str,
    ) {
        assert_eq!(
            most_recent_us_stock_minute_ingest_cutoff(date(now)),
            Some(date(expected)),
        );
    }

    #[rstest]
    #[case::open_window_always_runs("2040-01-02T09:20:00Z", Some("2040-01-07T01:04:00Z"), true)]
    #[case::closed_completed_range_skips(
        "2040-01-03T01:21:00Z",
        Some("2040-01-03T01:04:00Z"),
        false
    )]
    #[case::closed_range_one_minute_short_continues(
        "2040-01-03T01:21:00Z",
        Some("2040-01-03T01:03:00Z"),
        true
    )]
    #[case::closed_untracked_range_continues("2040-01-03T01:21:00Z", None, true)]
    fn minute_ingestion_skips_only_completed_closed_ranges(
        #[case] now: &str,
        #[case] completed_through: Option<&str>,
        #[case] expected: bool,
    ) {
        let completed_through = completed_through.map(date);

        assert_eq!(
            should_ingest_us_stock_minute_bars(date(now), completed_through),
            expected,
        );
    }
}
