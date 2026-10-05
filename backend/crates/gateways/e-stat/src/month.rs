use chrono::{Datelike, Months, NaiveDate};

pub(crate) fn first_day_of_month(date: NaiveDate) -> Option<NaiveDate> {
    NaiveDate::from_ymd_opt(date.year(), date.month(), 1)
}

pub(crate) fn last_day_of_month(month: NaiveDate) -> Option<NaiveDate> {
    next_month(month)?.pred_opt()
}

pub(crate) fn next_month(month: NaiveDate) -> Option<NaiveDate> {
    month.checked_add_months(Months::new(1))
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;

    use super::{first_day_of_month, last_day_of_month, next_month};

    #[test]
    fn month_boundaries_cover_year_transition_and_leap_day() {
        let actual = (
            first_day_of_month(NaiveDate::from_ymd_opt(2026, 12, 13).expect("date")),
            last_day_of_month(NaiveDate::from_ymd_opt(2028, 2, 1).expect("date")),
            next_month(NaiveDate::from_ymd_opt(2026, 12, 1).expect("date")),
        );

        assert_eq!(
            actual,
            (
                Some(NaiveDate::from_ymd_opt(2026, 12, 1).expect("date")),
                Some(NaiveDate::from_ymd_opt(2028, 2, 29).expect("date")),
                Some(NaiveDate::from_ymd_opt(2027, 1, 1).expect("date")),
            ),
        );
    }
}
