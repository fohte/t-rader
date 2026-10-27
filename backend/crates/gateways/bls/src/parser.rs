use chrono::{NaiveDate, NaiveTime};
use core_application::{
    calendar::macro_reference_targets::MacroReferenceTarget,
    macro_reference_source::MacroOfficialReference,
};
use core_domain::calendar_event::CalendarEventReferenceKind;
use reqwest::Url;
use scraper::{Html, Selector};

pub(super) struct BlsReleaseRow {
    pub release_date: NaiveDate,
    pub title: String,
    pub url: String,
}

pub(super) struct BlsPublication {
    pub release_date: NaiveDate,
    pub release_time: NaiveTime,
    pub period_start: NaiveDate,
    pub period_end: NaiveDate,
}

pub(super) fn parse_fred_external_id(external_id: &str) -> Option<(&str, NaiveDate)> {
    let (release_id, release_date) = external_id.split_once(':')?;
    if !matches!(release_id, "10" | "50") || release_date.len() != 10 {
        return None;
    }
    Some((
        release_id,
        NaiveDate::parse_from_str(release_date, "%Y-%m-%d").ok()?,
    ))
}

pub(super) fn parse_bls_archive_rows(
    html: &str,
    base_url: &str,
    release_id: &str,
) -> Option<Vec<BlsReleaseRow>> {
    let document = Html::parse_document(html);
    let link_selector = Selector::parse("a[href]").ok()?;
    let base = Url::parse(base_url).ok()?;
    let expected_prefix = match release_id {
        "10" => "cpi_",
        "50" => "empsit_",
        _ => return Some(Vec::new()),
    };
    Some(
        document
            .select(&link_selector)
            .filter_map(|link| {
                let href = link.value().attr("href")?;
                let filename = href.rsplit('/').next()?;
                if !filename.starts_with(expected_prefix) || !filename.ends_with(".htm") {
                    return None;
                }
                let date_part = filename
                    .strip_prefix(expected_prefix)?
                    .strip_suffix(".htm")?;
                let release_date = NaiveDate::parse_from_str(date_part, "%m%d%Y").ok()?;
                Some(BlsReleaseRow {
                    release_date,
                    title: link.text().collect::<String>().trim().to_owned(),
                    url: resolve_official_url(&base, href)?,
                })
            })
            .collect(),
    )
}

fn resolve_official_url(base: &Url, href: &str) -> Option<String> {
    let url = base.join(href).ok()?;
    (url.origin() == base.origin()
        && url.scheme() == "https"
        && url.port_or_known_default() == Some(443)
        && url.username().is_empty()
        && url.password().is_none())
    .then(|| url.to_string())
}

pub(super) fn parse_bls_publication(html: &str, release_id: &str) -> Option<BlsPublication> {
    let document = Html::parse_document(html);
    let body_selector = Selector::parse("body").ok()?;
    let text = document
        .select(&body_selector)
        .flat_map(|body| body.text())
        .collect::<Vec<_>>()
        .join("\n");
    let lowercase = text.to_ascii_lowercase();
    let embargo_start = lowercase.find("embargoed until")? + "embargoed until".len();
    let embargo = &lowercase[embargo_start..];
    let timezone_end = embargo.find("(et)")?;
    let release_time = parse_bls_time(&embargo[..timezone_end])?;
    let release_date = parse_month_day_year(&embargo[timezone_end + 4..])?;
    let period_marker = match release_id {
        "10" => "consumer price index",
        "50" => "employment situation",
        _ => return None,
    };
    if !lowercase.contains(period_marker) {
        return None;
    }
    let (year, month) = lowercase.lines().find_map(|line| {
        line.contains(period_marker)
            .then(|| parse_month_year(line))
            .flatten()
    })?;
    Some(BlsPublication {
        release_date,
        release_time,
        period_start: NaiveDate::from_ymd_opt(year, month, 1)?,
        period_end: last_day_of_month(year, month)?,
    })
}

pub(super) fn bls_document(
    target: &MacroReferenceTarget,
    release_id: &str,
    release_date: NaiveDate,
    row: &BlsReleaseRow,
    publication: &BlsPublication,
) -> Option<MacroOfficialReference> {
    if row.release_date != release_date
        || publication.release_date != release_date
        || row.title.trim().is_empty()
    {
        return None;
    }
    let source = match release_id {
        "10" | "50" => "bls",
        _ => return None,
    };
    Some(MacroOfficialReference {
        target: target.clone(),
        kind: CalendarEventReferenceKind::IndividualDocument,
        source: source.to_owned(),
        external_id: row.url.clone(),
        title: row.title.clone(),
        url: Some(row.url.clone()),
        target_period_start: Some(publication.period_start),
        target_period_end: Some(publication.period_end),
        target_period_type: Some("month".to_owned()),
        publication_evidence: Some(format!(
            "BLS release page states embargo until {} ET on {}",
            publication.release_time.format("%H:%M"),
            publication.release_date
        )),
        publication_evidence_url: Some(row.url.clone()),
    })
}

fn parse_month_year(text: &str) -> Option<(i32, u32)> {
    let months = [
        "january",
        "february",
        "march",
        "april",
        "may",
        "june",
        "july",
        "august",
        "september",
        "october",
        "november",
        "december",
    ];
    months.iter().enumerate().find_map(|(index, month)| {
        let month_position = text.find(month)?;
        let year = text[month_position + month.len()..]
            .split(|character: char| !character.is_ascii_digit())
            .find(|part| part.len() == 4)?
            .parse::<i32>()
            .ok()?;
        Some((year, u32::try_from(index + 1).ok()?))
    })
}

fn parse_bls_time(text: &str) -> Option<NaiveTime> {
    let time_text = text.split_whitespace().next()?;
    let (hour, minute) = time_text.split_once(':')?;
    let mut hour = hour.parse::<u32>().ok()?;
    let minute = minute.parse::<u32>().ok()?;
    let is_pm = text.contains("p.m.");
    let is_am = text.contains("a.m.");
    if (!is_am && !is_pm) || !(1..=12).contains(&hour) {
        return None;
    }
    if is_pm && hour < 12 {
        hour += 12;
    } else if is_am && hour == 12 {
        hour = 0;
    }
    NaiveTime::from_hms_opt(hour, minute, 0)
}

fn parse_month_day_year(text: &str) -> Option<NaiveDate> {
    let months = [
        "january",
        "february",
        "march",
        "april",
        "may",
        "june",
        "july",
        "august",
        "september",
        "october",
        "november",
        "december",
    ];
    months.iter().find_map(|month| {
        let month_position = text.find(month)?;
        let value = text[month_position..]
            .split_whitespace()
            .take(3)
            .collect::<Vec<_>>()
            .join(" ");
        let value = value
            .trim_matches(|character: char| !character.is_ascii_alphanumeric() && character != ',');
        NaiveDate::parse_from_str(value, "%B %d, %Y").ok()
    })
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
    use core_application::{
        calendar::macro_reference_targets::MacroReferenceTarget,
        macro_reference_source::MacroOfficialReference,
    };
    use core_domain::calendar_event::CalendarEventReferenceKind;
    use rstest::rstest;

    use super::{bls_document, parse_bls_archive_rows, parse_bls_publication};

    fn target(release_id: &str) -> MacroReferenceTarget {
        MacroReferenceTarget {
            event_source: "fred".to_owned(),
            event_external_id: format!("{release_id}:2099-04-08"),
            event_title: "Synthetic release event".to_owned(),
            same_day_event_count: 1,
        }
    }

    #[test]
    fn archive_parser_ignores_links_outside_the_source_origin() {
        let html = r#"<a href="https://attacker.invalid/cpi_04082099.htm">Synthetic release</a>"#;
        let actual = parse_bls_archive_rows(html, "https://example.invalid/archive", "10")
            .map(|rows| rows.len());

        assert_eq!(actual, Some(0));
    }

    #[rstest]
    #[case::consumer_price(
        "10",
        "cpi_04082099.htm",
        "Synthetic CPI archive row",
        "CONSUMER PRICE INDEX - MARCH 2099"
    )]
    #[case::employment(
        "50",
        "empsit_04082099.htm",
        "Synthetic employment archive row",
        "THE EMPLOYMENT SITUATION - MARCH 2099"
    )]
    fn bls_document_matches_the_et_date_time_and_target_period(
        #[case] release_id: &str,
        #[case] filename: &str,
        #[case] listing_title: &str,
        #[case] release_heading: &str,
    ) {
        let archive = format!("<a href=\"/news.release/archives/{filename}\">{listing_title}</a>");
        let release = format!(
            "<main><p>Transmission is embargoed until 8:30 a.m. (ET) Friday, April 8, 2099.</p><pre>{release_heading}</pre></main>"
        );
        let target = target(release_id);
        let archive_url = "https://example.invalid/archive";
        let actual = parse_bls_archive_rows(&archive, archive_url, release_id)
            .and_then(|rows| rows.into_iter().next())
            .and_then(|row| {
                let publication = parse_bls_publication(&release, release_id)?;
                bls_document(
                    &target,
                    release_id,
                    NaiveDate::from_ymd_opt(2099, 4, 8)?,
                    &row,
                    &publication,
                )
            });
        let expected_url = format!("https://example.invalid/news.release/archives/{filename}");

        assert_eq!(
            actual,
            Some(MacroOfficialReference {
                target,
                kind: CalendarEventReferenceKind::IndividualDocument,
                source: "bls".to_owned(),
                external_id: expected_url.clone(),
                title: listing_title.to_owned(),
                url: Some(expected_url.clone()),
                target_period_start: NaiveDate::from_ymd_opt(2099, 3, 1),
                target_period_end: NaiveDate::from_ymd_opt(2099, 3, 31),
                target_period_type: Some("month".to_owned()),
                publication_evidence: Some(
                    "BLS release page states embargo until 08:30 ET on 2099-04-08".to_owned(),
                ),
                publication_evidence_url: Some(expected_url),
            }),
        );
    }

    #[rstest]
    #[case::consumer_price("10", "<main><p>Embargoed until 8:30 a.m. (ET) Friday, April 8, 2099.</p><pre>Consumer Price Index - March 2099</pre></main>", Some(("2099-04-08", "08:30:00", "2099-03-01", "2099-03-31")))]
    #[case::different_release_type(
        "10",
        "<main><p>Embargoed until 8:30 a.m. (ET) Friday, April 8, 2099.</p><pre>Employment Situation - March 2099</pre></main>",
        None
    )]
    fn bls_page_requires_release_type_and_explicit_target_period(
        #[case] release_id: &str,
        #[case] html: &str,
        #[case] expected: Option<(&str, &str, &str, &str)>,
    ) {
        let actual = parse_bls_publication(html, release_id).map(|publication| {
            (
                publication.release_date.to_string(),
                publication.release_time.to_string(),
                publication.period_start.to_string(),
                publication.period_end.to_string(),
            )
        });

        assert_eq!(
            actual,
            expected.map(|values| {
                (
                    values.0.to_owned(),
                    values.1.to_owned(),
                    values.2.to_owned(),
                    values.3.to_owned(),
                )
            })
        );
    }
}
