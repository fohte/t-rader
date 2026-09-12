use chrono::{Datelike, Duration, NaiveDate, Weekday};

/// 東証の年末年始休場日 (12/31 〜 1/3)。法定の祝日ではなく取引所独自のルールのため
/// `jpholiday` ではカバーされず、ここで直接判定する。
fn is_tse_year_end_closure(date: NaiveDate) -> bool {
    matches!(
        (date.month(), date.day()),
        (12, 31) | (1, 1) | (1, 2) | (1, 3)
    )
}

/// `date` が日本の祝日 (振替休日含む) かどうかを判定する。
///
/// `chrono::NaiveDate` は常に実在する日付なので `jpholiday::Date` への変換が
/// 失敗することは通常ないが、clippy で `unwrap`/`expect` が deny のため、万一
/// 失敗した場合は「祝日ではない」として扱う (fail-open)。この関数の戻り値は
/// `latest_business_day` の上限の見積もりにのみ使われ、休場日を誤って営業日と
/// みなしても実害は無駄な再取得に留まるため、fail-open で問題ない。
fn is_jp_holiday(date: NaiveDate) -> bool {
    jpholiday::Date::new(date.year(), date.month(), date.day())
        .map(jpholiday::is_holiday)
        .unwrap_or(false)
}

/// `date` 以前で直近の証券取引所の営業日を返す。土日・日本の祝日 (振替休日含む)・
/// 東証の年末年始休場日 (12/31 〜 1/3) を休場日として扱い、該当する間は 1 日ずつ
/// 遡る。それ以外の東証独自の臨時休場等までは考慮しないため、実際の最終取引日
/// より新しい日付を返すことがまれにありうる。この関数の戻り値は「これ以降の
/// データはまだ存在しないはず」という上限の見積もりにしか使われないため、誤差の
/// 影響はその日に対応する bar が存在せず再取得を試みる (無害だが無駄になる)
/// 程度に留まる。
pub(crate) fn latest_business_day(date: NaiveDate) -> NaiveDate {
    let mut date = date;
    loop {
        let is_closed = matches!(date.weekday(), Weekday::Sat | Weekday::Sun)
            || is_tse_year_end_closure(date)
            || is_jp_holiday(date);
        if !is_closed {
            return date;
        }
        date -= Duration::days(1);
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case::saturday(
        NaiveDate::from_ymd_opt(2025, 6, 7).expect("date"),
        NaiveDate::from_ymd_opt(2025, 6, 6).expect("date")
    )]
    #[case::sunday(
        NaiveDate::from_ymd_opt(2025, 6, 8).expect("date"),
        NaiveDate::from_ymd_opt(2025, 6, 6).expect("date")
    )]
    #[case::weekday(
        NaiveDate::from_ymd_opt(2025, 6, 9).expect("date"),
        NaiveDate::from_ymd_opt(2025, 6, 9).expect("date")
    )]
    // 建国記念の日 (火)。前日の月曜は平日なので 1 日だけ遡る
    #[case::national_holiday(
        NaiveDate::from_ymd_opt(2025, 2, 11).expect("date"),
        NaiveDate::from_ymd_opt(2025, 2, 10).expect("date")
    )]
    // 文化の日 (2024-11-03 日) が日曜と重なった振替休日 (2024-11-04 月)。
    // 振替休日 (月) → 日 → 土 と 3 日連続で休場が続くケース
    #[case::substitute_holiday_chained_with_weekend(
        NaiveDate::from_ymd_opt(2024, 11, 4).expect("date"),
        NaiveDate::from_ymd_opt(2024, 11, 1).expect("date")
    )]
    // 東証の年末年始休場 (12/31 〜 1/3)。元日の祝日とも重なり計 4 日連続で休場が続く
    #[case::tse_year_end_closure(
        NaiveDate::from_ymd_opt(2025, 1, 3).expect("date"),
        NaiveDate::from_ymd_opt(2024, 12, 30).expect("date")
    )]
    fn latest_business_day_steps_back_over_market_closures(
        #[case] date: NaiveDate,
        #[case] expected: NaiveDate,
    ) {
        assert_eq!(latest_business_day(date), expected);
    }
}
