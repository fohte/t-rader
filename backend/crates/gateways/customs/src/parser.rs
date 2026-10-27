use chrono::NaiveDate;
use core_application::{
    calendar::macro_reference_targets::MacroReferenceTarget,
    macro_reference_source::MacroOfficialReference,
};
use core_domain::calendar_event::CalendarEventReferenceKind;
use reqwest::Url;
use scraper::{Html, Selector};

pub(super) struct JapaneseReleaseRow {
    pub publication_date: Option<NaiveDate>,
    pub period: Option<(NaiveDate, NaiveDate, String)>,
    pub title: String,
    pub url: Option<String>,
}

pub(super) fn parse_customs_rows(html: &str, base_url: &str) -> Option<Vec<JapaneseReleaseRow>> {
    let document = Html::parse_document(html);
    let row_selector = Selector::parse("tr").ok()?;
    let cell_selector = Selector::parse("td").ok()?;
    let link_selector = Selector::parse("a[href]").ok()?;
    let base = Url::parse(base_url).ok()?;
    let rows = document
        .select(&row_selector)
        .filter_map(|row| {
            let cells = row.select(&cell_selector).collect::<Vec<_>>();
            let period_text = cells.first()?.text().collect::<String>().trim().to_owned();
            let publication_text = cells.get(1)?.text().collect::<String>().trim().to_owned();
            let publication_date = parse_japanese_date(&publication_text);
            if period_text.is_empty() {
                return None;
            }
            let links = row
                .select(&link_selector)
                .filter_map(|link| {
                    let href = link.value().attr("href")?;
                    let is_document = href.to_ascii_lowercase().ends_with(".pdf");
                    is_document.then(|| resolve_official_url(&base, href))?
                })
                .collect::<Vec<_>>();
            Some(JapaneseReleaseRow {
                publication_date,
                period: parse_japanese_period(&period_text),
                title: period_text,
                url: (links.len() == 1)
                    .then(|| links.into_iter().next())
                    .flatten(),
            })
        })
        .collect::<Vec<_>>();
    (!rows.is_empty()).then_some(rows)
}

fn resolve_official_url(base: &Url, href: &str) -> Option<String> {
    let url = base.join(href).ok()?;
    (base.scheme() == "https"
        && url.scheme() == "https"
        && url.origin() == base.origin()
        && url.username().is_empty()
        && url.password().is_none())
    .then(|| url.to_string())
}

pub(super) fn customs_document(
    target: &MacroReferenceTarget,
    row: &JapaneseReleaseRow,
    archive_url: &str,
) -> Option<MacroOfficialReference> {
    let publication_date = row.publication_date?;
    let (period_start, period_end, period_type) = row.period.as_ref()?;
    let url = row.url.as_ref()?;
    Some(MacroOfficialReference {
        target: target.clone(),
        kind: CalendarEventReferenceKind::IndividualDocument,
        source: "customs_jp".to_owned(),
        external_id: url.clone(),
        title: row.title.clone(),
        url: Some(url.clone()),
        target_period_start: Some(*period_start),
        target_period_end: Some(*period_end),
        target_period_type: Some(period_type.clone()),
        publication_evidence: Some(format!(
            "Trade Statistics archive row dated {publication_date} for {}",
            row.title
        )),
        publication_evidence_url: Some(archive_url.to_owned()),
    })
}

fn parse_japanese_date(value: &str) -> Option<NaiveDate> {
    let year_position = value.find('年')?;
    let year_value = numbers_before_marker(value, '年').first().copied()?;
    let year = to_gregorian_year(year_value, &value[..year_position])?;
    let month = numbers_before_marker(value, '月').first().copied()?;
    let day = numbers_before_marker(value, '日').first().copied()?;
    NaiveDate::from_ymd_opt(year, month, day)
}

fn parse_japanese_period(value: &str) -> Option<(NaiveDate, NaiveDate, String)> {
    let years = years_before_marker(value);
    let months = numbers_before_marker(value, '月');
    let first_year = *years.first()?;
    if months.is_empty() {
        let (start_month, start_year, end_month, end_year, period_type) =
            if value.contains("上半期") {
                (4, first_year, 9, first_year, "half_year")
            } else if value.contains("下半期") {
                (10, first_year, 3, first_year.checked_add(1)?, "half_year")
            } else if value.contains("年度") {
                (4, first_year, 3, first_year.checked_add(1)?, "fiscal_year")
            } else {
                (1, first_year, 12, first_year, "year")
            };
        let start = NaiveDate::from_ymd_opt(start_year, start_month, 1)?;
        let end = last_day_of_month(end_year, end_month)?;
        return Some((start, end, period_type.to_owned()));
    }
    let first_month = months[0];
    let start = NaiveDate::from_ymd_opt(first_year, first_month, 1)?;
    let (end_year, end_month) = if months.len() > 1 {
        let candidate_year = years.last().copied().unwrap_or(years[0]);
        let last_month = *months.last()?;
        let end_year = if years.len() == 1 && last_month < first_month {
            candidate_year.checked_add(1)?
        } else {
            candidate_year
        };
        (end_year, last_month)
    } else {
        (first_year, first_month)
    };
    let end = last_day_of_month(end_year, end_month)?;
    let period_type = if value.contains("上半期") || value.contains("下半期") {
        "half_year"
    } else if value.contains("年度") {
        "fiscal_year"
    } else if months.len() > 1 {
        "month_range"
    } else {
        "month"
    };
    Some((start, end, period_type.to_owned()))
}

fn years_before_marker(value: &str) -> Vec<i32> {
    value
        .match_indices('年')
        .filter_map(|(position, _)| {
            let marker_end = position + '年'.len_utf8();
            let year = numbers_before_marker(&value[..marker_end], '年')
                .first()
                .copied()?;
            to_gregorian_year(year, &value[..position])
        })
        .collect()
}

fn numbers_before_marker(value: &str, marker: char) -> Vec<u32> {
    value
        .match_indices(marker)
        .filter_map(|(position, _)| {
            let digits = value[..position]
                .chars()
                .rev()
                .take_while(char::is_ascii_digit)
                .collect::<String>()
                .chars()
                .rev()
                .collect::<String>();
            digits
                .parse::<u32>()
                .ok()
                .or_else(|| (marker == '年' && value[..position].ends_with('元')).then_some(1))
        })
        .collect()
}

fn to_gregorian_year(year: u32, prefix: &str) -> Option<i32> {
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

fn last_day_of_month(year: i32, month: u32) -> Option<NaiveDate> {
    let (next_year, next_month) = if month == 12 {
        (year.checked_add(1)?, 1)
    } else {
        (year, month + 1)
    };
    NaiveDate::from_ymd_opt(next_year, next_month, 1)?.pred_opt()
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;
    use indoc::indoc;

    use super::parse_customs_rows;

    #[test]
    fn fiscal_half_year_is_not_reported_as_a_full_year() {
        let html = indoc! {
            r#"
            <table><tbody>
              <tr><td>2099年度上半期</td><td>2099年10月8日</td><td><a href="/archive/sample.pdf">PDF</a></td></tr>
            </tbody></table>
            "#
        };

        let actual = parse_customs_rows(html, "https://example.invalid/archive")
            .and_then(|rows| rows.into_iter().next())
            .and_then(|row| row.period);

        assert_eq!(
            actual,
            Some((
                NaiveDate::from_ymd_opt(2099, 4, 1).unwrap_or(NaiveDate::MIN),
                NaiveDate::from_ymd_opt(2099, 9, 30).unwrap_or(NaiveDate::MIN),
                "half_year".to_owned(),
            )),
        );
    }

    #[test]
    fn customs_archive_does_not_treat_external_pdf_as_a_document() {
        let html = indoc! {
            r#"
            <table><tbody>
              <tr><td>2099年4月</td><td>2099年5月8日</td><td><a href="https://attacker.invalid/archive/sample.pdf">資料</a></td></tr>
            </tbody></table>
            "#
        };
        let actual = parse_customs_rows(html, "https://example.invalid/archive")
            .map(|rows| rows.into_iter().map(|row| row.url).collect::<Vec<_>>());

        assert_eq!(actual, Some(vec![None]));
    }
}
