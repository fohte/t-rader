use chrono::{Datelike, Duration, NaiveDate, Weekday};

/// `date` が土日ならその直前の金曜日を返す。祝日は考慮しない。
pub(crate) fn latest_business_day(date: NaiveDate) -> NaiveDate {
    match date.weekday() {
        Weekday::Sat => date - Duration::days(1),
        Weekday::Sun => date - Duration::days(2),
        _ => date,
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case::saturday(
        NaiveDate::from_ymd_opt(2025, 1, 4).expect("date"),
        NaiveDate::from_ymd_opt(2025, 1, 3).expect("date")
    )]
    #[case::sunday(
        NaiveDate::from_ymd_opt(2025, 1, 5).expect("date"),
        NaiveDate::from_ymd_opt(2025, 1, 3).expect("date")
    )]
    #[case::weekday(
        NaiveDate::from_ymd_opt(2025, 1, 6).expect("date"),
        NaiveDate::from_ymd_opt(2025, 1, 6).expect("date")
    )]
    fn latest_business_day_steps_back_from_weekends(
        #[case] date: NaiveDate,
        #[case] expected: NaiveDate,
    ) {
        assert_eq!(latest_business_day(date), expected);
    }
}
