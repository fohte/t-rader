use chrono::NaiveDate;
use core_application::calendar::macro_reference_targets::MacroReferenceTarget;
use scraper::ElementRef;

pub(super) fn target_publication_date(
    target: &MacroReferenceTarget,
    expected_stat_code: &str,
) -> Option<NaiveDate> {
    if target.event_source != "e_stat" {
        return None;
    }
    let (stat_code, datetime) = target.event_external_id.split_once(':')?;
    if stat_code != expected_stat_code {
        return None;
    }
    Some(
        chrono::NaiveDateTime::parse_from_str(datetime, "%Y%m%d%H%M")
            .ok()?
            .date(),
    )
}

pub(super) fn element_text(element: &ElementRef<'_>) -> String {
    element
        .text()
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

pub(super) fn parse_japanese_date(value: &str) -> Option<NaiveDate> {
    value.match_indices('日').find_map(|(day_position, _)| {
        let before_day = &value[..day_position];
        let day = trailing_number(before_day)?;
        let month_position = before_day.rfind('月')?;
        let before_month = &before_day[..month_position];
        let month = trailing_number(before_month)?;
        let year_position = before_month.rfind('年')?;
        let before_year = &before_month[..year_position];
        let year_value = trailing_number(before_year)?;
        let year = to_gregorian_year(year_value, before_year)?;
        NaiveDate::from_ymd_opt(year, month, day)
    })
}

pub(super) fn trailing_number(value: &str) -> Option<u32> {
    let digits = value
        .chars()
        .rev()
        .take_while(char::is_ascii_digit)
        .collect::<String>()
        .chars()
        .rev()
        .collect::<String>();
    if digits.is_empty() {
        value.ends_with("元").then_some(1)
    } else {
        digits.parse().ok()
    }
}

pub(super) fn leading_number(value: &str) -> Option<(u32, &str)> {
    let end = value
        .char_indices()
        .find_map(|(position, character)| (!character.is_ascii_digit()).then_some(position))
        .unwrap_or(value.len());
    if end == 0 {
        return None;
    }
    Some((value[..end].parse().ok()?, &value[end..]))
}

pub(super) fn to_gregorian_year(year: u32, prefix: &str) -> Option<i32> {
    let offset = if prefix.contains("令和") {
        2018
    } else if prefix.contains("平成") {
        1988
    } else if prefix.contains("昭和") {
        1925
    } else {
        return i32::try_from(year).ok();
    };
    i32::try_from(year).ok()?.checked_add(offset)
}

pub(super) fn last_day_of_month(year: i32, month: u32) -> Option<NaiveDate> {
    let (next_year, next_month) = if month == 12 {
        (year.checked_add(1)?, 1)
    } else {
        (year, month + 1)
    };
    NaiveDate::from_ymd_opt(next_year, next_month, 1)?.pred_opt()
}
