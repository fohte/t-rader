use std::str::FromStr;
use std::time::Duration;

use chrono::{DateTime, Utc};
use cron::Schedule;

pub(super) fn parse_schedule(expr: &str) -> Result<Schedule, cron::error::Error> {
    let trimmed = expr.trim();
    let field_count = trimmed.split_whitespace().count();
    let normalized = if field_count == 5 {
        format!("0 {trimmed}")
    } else {
        trimmed.to_string()
    };

    let mut fields: Vec<String> = normalized.split_whitespace().map(str::to_string).collect();
    if let Some(day_of_week) = fields.get_mut(5) {
        *day_of_week = convert_dow_field(day_of_week);
    }
    Schedule::from_str(&fields.join(" "))
}

pub(super) fn should_fire(
    schedule: &Schedule,
    last_fired_at: Option<DateTime<Utc>>,
    now: DateTime<Utc>,
    interval: Duration,
) -> bool {
    let after = last_fired_at.unwrap_or_else(|| {
        now - chrono::Duration::from_std(interval).unwrap_or(chrono::Duration::zero())
    });
    schedule
        .after(&after)
        .next()
        .is_some_and(|next| next <= now)
}

fn posix_to_quartz_dow(day: u32) -> u32 {
    if day <= 7 { day % 7 + 1 } else { day }
}

fn convert_dow_field(field: &str) -> String {
    field
        .split(',')
        .map(convert_dow_item)
        .collect::<Vec<_>>()
        .join(",")
}

fn convert_dow_item(item: &str) -> String {
    let (value, step) = match item.split_once('/') {
        Some((value, step)) => (value, Some(step)),
        None => (item, None),
    };
    match value.split_once('-') {
        Some((start, end)) => convert_dow_range(item, start, end, step),
        None => convert_dow_value(value, step),
    }
}

fn convert_dow_value(value: &str, step: Option<&str>) -> String {
    let converted = match value.parse::<u32>() {
        Ok(day) => posix_to_quartz_dow(day).to_string(),
        Err(_) => value.to_string(),
    };
    match step {
        Some(step) => format!("{converted}/{step}"),
        None => converted,
    }
}

fn convert_dow_range(original: &str, start: &str, end: &str, step: Option<&str>) -> String {
    let (Ok(start), Ok(end)) = (start.parse::<u32>(), end.parse::<u32>()) else {
        return original.to_string();
    };
    if start > end {
        return original.to_string();
    }
    let step = match step {
        Some(step) => match step.parse::<usize>() {
            Ok(step) if step > 0 => step,
            _ => return original.to_string(),
        },
        None => 1,
    };
    // POSIX の 0 と 7 は同じ日曜なので、端点変換で逆順になる範囲を曜日集合に展開する。
    (start..=end)
        .step_by(step)
        .map(posix_to_quartz_dow)
        .map(|day| day.to_string())
        .collect::<Vec<_>>()
        .join(",")
}

#[cfg(test)]
mod tests {
    use chrono::{DateTime, Utc};
    use rstest::rstest;

    use super::{parse_schedule, should_fire};

    const DEFAULT_INTERVAL: std::time::Duration = std::time::Duration::from_secs(60);

    fn timestamp(value: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(value)
            .expect("valid test timestamp")
            .with_timezone(&Utc)
    }

    #[rstest]
    #[case::five_field("0 9 * * 1-5")]
    #[case::leading_whitespace("  0 9 * * 1-5  ")]
    #[case::six_field("0 0 9 * * 1-5")]
    fn accepts_schedule_forms(#[case] expression: &str) {
        assert!(parse_schedule(expression).is_ok());
    }

    #[rstest]
    #[case::weekday_range("0 9 * * 1-5", "2026-01-04T00:00:00Z", "2026-01-05T09:00:00Z")]
    #[case::sunday_zero("0 9 * * 0", "2026-01-01T00:00:00Z", "2026-01-04T09:00:00Z")]
    #[case::sunday_seven("0 9 * * 7", "2026-01-01T00:00:00Z", "2026-01-04T09:00:00Z")]
    #[case::alpha_range("0 9 * * MON-FRI", "2026-01-04T00:00:00Z", "2026-01-05T09:00:00Z")]
    #[case::range_crossing_sunday_includes_friday(
        "0 9 * * 5-7",
        "2026-01-08T00:00:00Z",
        "2026-01-09T09:00:00Z"
    )]
    #[case::range_crossing_sunday_skips_weekdays(
        "0 9 * * 5-7",
        "2026-01-11T10:00:00Z",
        "2026-01-16T09:00:00Z"
    )]
    fn interprets_day_of_week_as_posix(
        #[case] expression: &str,
        #[case] after: &str,
        #[case] expected: &str,
    ) {
        let next = parse_schedule(expression)
            .expect("valid schedule")
            .after(&timestamp(after))
            .next();
        assert_eq!(next, Some(timestamp(expected)));
    }

    #[rstest]
    #[case::first_slot_in_window("* * * * *", None, "2026-01-01T09:00:00Z", DEFAULT_INTERVAL, true)]
    #[case::already_fired(
        "0 9 * * *",
        Some("2026-01-01T09:00:00Z"),
        "2026-01-01T09:30:00Z",
        DEFAULT_INTERVAL,
        false
    )]
    #[case::next_slot_pending(
        "0 9,10 * * *",
        Some("2026-01-01T09:00:00Z"),
        "2026-01-01T09:30:00Z",
        DEFAULT_INTERVAL,
        false
    )]
    #[case::next_slot_passed(
        "0 9,10 * * *",
        Some("2026-01-01T09:00:00Z"),
        "2026-01-01T10:30:00Z",
        DEFAULT_INTERVAL,
        true
    )]
    #[case::default_interval_window(
        "0 9 * * *",
        None,
        "2026-01-01T09:00:30Z",
        DEFAULT_INTERVAL,
        true
    )]
    #[case::short_interval_window(
        "0 9 * * *",
        None,
        "2026-01-01T09:00:30Z",
        std::time::Duration::from_secs(10),
        false
    )]
    fn evaluates_due_slot(
        #[case] expression: &str,
        #[case] last_fired_at: Option<&str>,
        #[case] now: &str,
        #[case] interval: std::time::Duration,
        #[case] expected: bool,
    ) {
        let last_fired_at = last_fired_at.map(timestamp);
        let due = should_fire(
            &parse_schedule(expression).expect("valid schedule"),
            last_fired_at,
            timestamp(now),
            interval,
        );
        assert_eq!(due, expected);
    }
}
