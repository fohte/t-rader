use std::io::Cursor;

use calamine::{Data, Reader, Xlsx, open_workbook_from_rs};
use chrono::{Datelike, NaiveDate, NaiveDateTime};
use core_application::{
    calendar::macro_reference_targets::MacroReferenceTarget,
    macro_reference_source::MacroOfficialReference,
};
use core_domain::calendar_event::CalendarEventReferenceKind;
use reqwest::Url;
use scraper::{Html, Selector};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct CensusArchiveLink {
    pub title: String,
    pub period_start: Option<NaiveDate>,
    pub url: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct CensusPdfPublication {
    pub period_start: NaiveDate,
    pub period_end: NaiveDate,
    pub release_date: NaiveDate,
}

pub(super) fn parse_archive_links(html: &str, base_url: &str) -> Option<Vec<CensusArchiveLink>> {
    let document = Html::parse_document(html);
    let selector = Selector::parse("a[href]").ok()?;
    let base = Url::parse(base_url).ok()?;
    Some(
        document
            .select(&selector)
            .filter_map(|link| {
                let href = link.value().attr("href")?;
                let normalized = href.to_ascii_lowercase();
                let path = normalized.split(['?', '#']).next()?;
                if !(path.ends_with(".pdf") || path.ends_with(".xlsx") || path.ends_with(".xls")) {
                    return None;
                }
                let title = link.text().collect::<String>().trim().to_owned();
                let period_start = parse_period_month(&format!("{title} {href}"));
                Some(CensusArchiveLink {
                    title,
                    period_start,
                    url: resolve_archive_url(&base, href)?,
                })
            })
            .collect(),
    )
}

fn resolve_archive_url(base: &Url, href: &str) -> Option<String> {
    let url = base.join(href).ok()?;
    let host = url.host_str()?;
    let base_host = base.host_str()?;
    let is_census_host = host == "census.gov" || host.ends_with(".census.gov");
    (url.scheme() == "https"
        && url.port_or_known_default() == Some(443)
        && url.username().is_empty()
        && url.password().is_none()
        && (host == base_host || is_census_host))
        .then(|| url.to_string())
}

pub(super) fn parse_release_date_workbook(bytes: &[u8]) -> Option<Vec<NaiveDate>> {
    let mut workbook: Xlsx<_> = open_workbook_from_rs(Cursor::new(bytes)).ok()?;
    let sheet_names = workbook.sheet_names();
    let mut release_dates = Vec::new();
    let mut found_month_headers = false;

    for sheet_name in sheet_names {
        let range = workbook.worksheet_range(&sheet_name).ok()?;
        let rows = range.rows().collect::<Vec<_>>();
        let Some(sheet_dates) = parse_release_date_rows(&rows) else {
            continue;
        };
        found_month_headers = true;
        release_dates.extend(sheet_dates);
    }

    release_dates.sort_unstable();
    release_dates.dedup();
    (found_month_headers && !release_dates.is_empty()).then_some(release_dates)
}

fn parse_release_date_rows(rows: &[&[Data]]) -> Option<Vec<NaiveDate>> {
    let (header_index, month_columns) = rows.iter().enumerate().find_map(|(index, row)| {
        let month_columns = row
            .iter()
            .enumerate()
            .filter_map(|(column, cell)| parse_month_header(cell).map(|month| (column, month)))
            .collect::<Vec<_>>();
        let mut unique_months = month_columns
            .iter()
            .map(|(_, month)| *month)
            .collect::<Vec<_>>();
        unique_months.sort_unstable();
        unique_months.dedup();
        (unique_months.len() == 12).then_some((index, month_columns))
    })?;

    let mut release_dates = Vec::new();
    for row in rows.iter().skip(header_index + 1) {
        let Some(year) = row.first().and_then(parse_year_cell) else {
            continue;
        };
        for (column, month) in &month_columns {
            let Some(day) = row.get(*column).and_then(parse_release_day_cell) else {
                continue;
            };
            if let Some(release_date) = NaiveDate::from_ymd_opt(year, *month, day) {
                release_dates.push(release_date);
            }
        }
    }
    (!release_dates.is_empty()).then_some(release_dates)
}

pub(super) fn parse_pdf_publication(bytes: &[u8]) -> Option<CensusPdfPublication> {
    let text = pdf_extract::extract_text_from_mem(bytes).ok()?;
    parse_pdf_text(&text)
}

pub(super) fn parse_pdf_text(text: &str) -> Option<CensusPdfPublication> {
    let lines = text.lines().collect::<Vec<_>>();
    let mut release_dates = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        let lowercase = line.to_ascii_lowercase();
        if lowercase.contains("for release") {
            let context = lines[index..(index + 3).min(lines.len())].join(" ");
            if let Some(date) = parse_full_date(&context) {
                release_dates.push(date);
            }
        }
    }
    let mut distinct_release_dates = release_dates;
    distinct_release_dates.sort_unstable();
    distinct_release_dates.dedup();
    let [release_date] = distinct_release_dates.as_slice() else {
        return None;
    };

    let mut period_starts = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        let lowercase = line.to_ascii_lowercase();
        if !lowercase.contains("advance monthly sales")
            && !lowercase.contains("retail and food services")
        {
            continue;
        }
        let context = lines[index..(index + 4).min(lines.len())].join(" ");
        if let Some(period_start) = parse_period_month(&context) {
            period_starts.push(period_start);
        }
    }
    period_starts.sort_unstable();
    period_starts.dedup();
    let [period_start] = period_starts.as_slice() else {
        return None;
    };
    Some(CensusPdfPublication {
        period_start: *period_start,
        period_end: last_day_of_month(*period_start)?,
        release_date: *release_date,
    })
}

pub(super) fn census_document(
    target: &MacroReferenceTarget,
    link: &CensusArchiveLink,
    publication: &CensusPdfPublication,
) -> Option<MacroOfficialReference> {
    if link.period_start != Some(publication.period_start) || link.title.trim().is_empty() {
        return None;
    }
    Some(MacroOfficialReference {
        target: target.clone(),
        kind: CalendarEventReferenceKind::IndividualDocument,
        source: "census".to_owned(),
        external_id: link.url.clone(),
        title: link.title.clone(),
        url: Some(link.url.clone()),
        target_period_start: Some(publication.period_start),
        target_period_end: Some(publication.period_end),
        target_period_type: Some("month".to_owned()),
        publication_evidence: Some(format!(
            "Census release PDF states publication date {} for period {} through {}",
            publication.release_date, publication.period_start, publication.period_end
        )),
        publication_evidence_url: Some(link.url.clone()),
    })
}

fn parse_month_header(cell: &Data) -> Option<u32> {
    let value = cell.to_string().trim().to_ascii_lowercase();
    match value.as_str() {
        "jan" | "january" => Some(1),
        "feb" | "february" => Some(2),
        "mar" | "march" => Some(3),
        "apr" | "april" => Some(4),
        "may" => Some(5),
        "jun" | "june" => Some(6),
        "jul" | "july" => Some(7),
        "aug" | "august" => Some(8),
        "sep" | "sept" | "september" => Some(9),
        "oct" | "october" => Some(10),
        "nov" | "november" => Some(11),
        "dec" | "december" => Some(12),
        _ => None,
    }
}

fn parse_year_cell(cell: &Data) -> Option<i32> {
    let year = match cell {
        Data::Int(value) => i32::try_from(*value).ok()?,
        Data::Float(value) if value.fract() == 0.0 => format!("{value:.0}").parse().ok()?,
        Data::String(value) => value.trim().parse().ok()?,
        _ => return None,
    };
    (1900..=2200).contains(&year).then_some(year)
}

fn parse_release_day_cell(cell: &Data) -> Option<u32> {
    let day = match cell {
        Data::Int(value) => u32::try_from(*value).ok()?,
        Data::Float(value) if value.fract() == 0.0 => format!("{value:.0}").parse().ok()?,
        Data::String(value) => value.trim().parse().ok()?,
        _ => return None,
    };
    (1..=31).contains(&day).then_some(day)
}

fn parse_date_string(value: &str) -> Option<NaiveDate> {
    let value = value.trim();
    ["%Y-%m-%d", "%Y/%m/%d", "%m/%d/%Y", "%B %d, %Y", "%b %d, %Y"]
        .iter()
        .find_map(|format| NaiveDate::parse_from_str(value, format).ok())
        .or_else(|| {
            NaiveDateTime::parse_from_str(value, "%Y-%m-%dT%H:%M:%S")
                .ok()
                .map(|date_time| date_time.date())
        })
}

fn parse_period_month(value: &str) -> Option<NaiveDate> {
    if parse_date_string(value).is_some() {
        return None;
    }
    if let Some((year, month)) = parse_numeric_year_month(value) {
        return NaiveDate::from_ymd_opt(year, month, 1);
    }

    let lowercase = value.to_ascii_lowercase();
    let months = [
        ("january", 1),
        ("february", 2),
        ("march", 3),
        ("april", 4),
        ("may", 5),
        ("june", 6),
        ("july", 7),
        ("august", 8),
        ("september", 9),
        ("october", 10),
        ("november", 11),
        ("december", 12),
        ("jan", 1),
        ("feb", 2),
        ("mar", 3),
        ("apr", 4),
        ("jun", 6),
        ("jul", 7),
        ("aug", 8),
        ("sep", 9),
        ("oct", 10),
        ("nov", 11),
        ("dec", 12),
    ];
    let mut candidates = Vec::new();
    for (month_name, month_number) in months {
        for (month_position, _) in lowercase.match_indices(month_name) {
            let before_is_letter = lowercase[..month_position]
                .chars()
                .next_back()
                .is_some_and(|character| character.is_ascii_alphabetic());
            let after_month = &lowercase[month_position + month_name.len()..];
            let after_is_letter = after_month
                .chars()
                .next()
                .is_some_and(|character| character.is_ascii_alphabetic());
            if before_is_letter || after_is_letter {
                continue;
            }
            let Some(year_text) = after_month
                .trim_start_matches(|character: char| !character.is_ascii_digit())
                .split(|character: char| !character.is_ascii_digit())
                .next()
                .filter(|part| part.len() == 4)
            else {
                continue;
            };
            let Ok(year) = year_text.parse::<i32>() else {
                continue;
            };
            if let Some(date) = NaiveDate::from_ymd_opt(year, month_number, 1) {
                candidates.push((month_position, date));
            }
        }
    }
    let mut period_starts = candidates
        .into_iter()
        .map(|(_, period_start)| period_start)
        .collect::<Vec<_>>();
    period_starts.sort_unstable();
    period_starts.dedup();
    let [period_start] = period_starts.as_slice() else {
        return None;
    };
    Some(*period_start)
}

fn parse_numeric_year_month(value: &str) -> Option<(i32, u32)> {
    for token in value.split(|character: char| {
        !character.is_ascii_digit() && character != '-' && character != '/'
    }) {
        let token = token.trim_matches(|character| character == '-' || character == '/');
        let Some((first, second)) = token.split_once('-').or_else(|| token.split_once('/')) else {
            continue;
        };
        if second.contains('-') || second.contains('/') {
            continue;
        }
        if first.len() == 4 && second.len() == 2 {
            let year = first.parse().ok()?;
            let month = second.parse().ok()?;
            return (1..=12).contains(&month).then_some((year, month));
        }
        if first.len() == 2 && second.len() == 4 {
            let month = first.parse().ok()?;
            let year = second.parse().ok()?;
            return (1..=12).contains(&month).then_some((year, month));
        }
    }
    None
}

fn parse_full_date(value: &str) -> Option<NaiveDate> {
    let lowercase = value.to_ascii_lowercase();
    let months = [
        ("january", 1),
        ("february", 2),
        ("march", 3),
        ("april", 4),
        ("may", 5),
        ("june", 6),
        ("july", 7),
        ("august", 8),
        ("september", 9),
        ("october", 10),
        ("november", 11),
        ("december", 12),
    ];
    months.iter().find_map(|(month, month_number)| {
        let position = lowercase.find(month)?;
        let candidate = value[position..]
            .split_whitespace()
            .take(3)
            .collect::<Vec<_>>();
        let [_, day_text, year_text] = candidate.as_slice() else {
            return None;
        };
        let day = day_text
            .trim_matches(|character: char| !character.is_ascii_digit())
            .parse::<u32>()
            .ok()?;
        let year_text = year_text.trim_matches(|character: char| !character.is_ascii_digit());
        if year_text.len() != 4 {
            return None;
        }
        let year = year_text.parse::<i32>().ok()?;
        NaiveDate::from_ymd_opt(year, *month_number, day)
    })
}

fn last_day_of_month(date: NaiveDate) -> Option<NaiveDate> {
    let (year, month) = if date.month() == 12 {
        (date.year().checked_add(1)?, 1)
    } else {
        (date.year(), date.month() + 1)
    };
    NaiveDate::from_ymd_opt(year, month, 1)?.pred_opt()
}

#[cfg(test)]
mod tests {
    use base64::{Engine, engine::general_purpose::STANDARD};
    use calamine::Data;
    use chrono::NaiveDate;
    use core_application::{
        calendar::macro_reference_targets::MacroReferenceTarget,
        macro_reference_source::MacroOfficialReference,
    };
    use core_domain::calendar_event::CalendarEventReferenceKind;
    use indoc::indoc;
    use rstest::rstest;

    use super::{
        CensusArchiveLink, CensusPdfPublication, census_document, parse_archive_links,
        parse_pdf_text, parse_period_month, parse_release_date_rows, parse_release_date_workbook,
    };

    #[test]
    fn archive_parser_ignores_links_outside_the_source_domain() {
        let html = r#"<a href="https://attacker.invalid/sample-report.pdf">Synthetic report</a>"#;

        assert_eq!(
            parse_archive_links(html, "https://example.invalid/archive"),
            Some(Vec::new()),
        );
    }

    #[rstest]
    #[case::full_month("September 2099", Some("2099-09-01"))]
    #[case::abbreviated_month("Sep 2099", Some("2099-09-01"))]
    #[case::numeric_month("2099-09", Some("2099-09-01"))]
    #[case::unclear_period("2099", None)]
    #[case::full_publication_date("April 8, 2099", None)]
    #[case::numeric_publication_date("2099-04-08", None)]
    fn period_parser_requires_a_month_and_year(
        #[case] input: &str,
        #[case] expected: Option<&str>,
    ) {
        let actual = parse_period_month(input).map(|date| date.to_string());
        assert_eq!(actual, expected.map(str::to_owned));
    }

    #[test]
    fn workbook_calendar_combines_year_month_and_release_day_cells() {
        let header = [
            "Year", "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov",
            "Dec",
        ]
        .map(|value| Data::String(value.to_owned()));
        let mut release_days = vec![Data::Empty; 13];
        release_days[0] = Data::Int(2099);
        release_days[4] = Data::Int(8);
        let rows = vec![header.as_slice(), release_days.as_slice()];

        assert_eq!(
            parse_release_date_rows(&rows),
            Some(vec![
                NaiveDate::from_ymd_opt(2099, 4, 8).unwrap_or(NaiveDate::MIN)
            ]),
        );
    }

    #[test]
    fn workbook_fixture_parses_the_year_month_day_matrix() {
        let workbook_bytes = STANDARD
            .decode(include_str!("../tests/fixtures/synthetic-release-dates.xlsx.b64").trim())
            .ok();
        let actual = workbook_bytes
            .as_deref()
            .and_then(parse_release_date_workbook);

        assert_eq!(
            actual,
            Some(vec![
                NaiveDate::from_ymd_opt(2099, 4, 8).unwrap_or(NaiveDate::MIN)
            ]),
        );
    }

    #[test]
    fn archive_parser_resolves_period_pdf_links() {
        let archive = indoc! {
            r#"
            <html><body>
              <a href="/retail/report-september-2099.pdf">Retail Sales, September 2099</a>
              <a href="/retail/release-dates.xlsx">Release dates</a>
            </body></html>
            "#
        };

        let actual = parse_archive_links(archive, "https://example.invalid/archive").map(|links| {
            links
                .into_iter()
                .map(|link| {
                    (
                        link.title,
                        link.period_start.map(|date| date.to_string()),
                        link.url,
                    )
                })
                .collect::<Vec<_>>()
        });
        assert_eq!(
            actual,
            Some(vec![
                (
                    "Retail Sales, September 2099".to_owned(),
                    Some("2099-09-01".to_owned()),
                    "https://example.invalid/retail/report-september-2099.pdf".to_owned(),
                ),
                (
                    "Release dates".to_owned(),
                    None,
                    "https://example.invalid/retail/release-dates.xlsx".to_owned(),
                ),
            ]),
        );
    }

    #[test]
    fn pdf_text_requires_explicit_release_date_and_period() {
        let release = indoc! {
            "FOR RELEASE: April 8, 2099\nAdvance Monthly Sales for Retail and Food Services\nMarch 2099\nRevised estimates were released on May 1, 2099"
        };
        let actual = parse_pdf_text(release).map(|publication| {
            (
                publication.period_start.to_string(),
                publication.period_end.to_string(),
                publication.release_date.to_string(),
            )
        });

        assert_eq!(
            actual,
            Some((
                "2099-03-01".to_owned(),
                "2099-03-31".to_owned(),
                "2099-04-08".to_owned(),
            )),
        );
        assert_eq!(
            parse_pdf_text("Advance Monthly Sales for Retail and Food Services, March 2099"),
            None,
        );
    }

    #[test]
    fn individual_document_requires_matching_workbook_and_pdf_evidence() {
        let target = MacroReferenceTarget {
            event_source: "fred".to_owned(),
            event_external_id: "9:2099-04-08".to_owned(),
            event_title: "Synthetic retail release event".to_owned(),
            same_day_event_count: 1,
        };
        let link = CensusArchiveLink {
            title: "Synthetic retail release report".to_owned(),
            period_start: NaiveDate::from_ymd_opt(2099, 3, 1),
            url: "https://example.invalid/retail/report.pdf".to_owned(),
        };
        let publication = CensusPdfPublication {
            period_start: NaiveDate::from_ymd_opt(2099, 3, 1).unwrap_or(NaiveDate::MIN),
            period_end: NaiveDate::from_ymd_opt(2099, 3, 31).unwrap_or(NaiveDate::MIN),
            release_date: NaiveDate::from_ymd_opt(2099, 4, 8).unwrap_or(NaiveDate::MIN),
        };
        let actual = census_document(&target, &link, &publication);

        assert_eq!(
            actual,
            Some(MacroOfficialReference {
                target,
                kind: CalendarEventReferenceKind::IndividualDocument,
                source: "census".to_owned(),
                external_id: "https://example.invalid/retail/report.pdf".to_owned(),
                title: "Synthetic retail release report".to_owned(),
                url: Some("https://example.invalid/retail/report.pdf".to_owned()),
                target_period_start: Some(NaiveDate::from_ymd_opt(2099, 3, 1)
                    .unwrap_or(NaiveDate::MIN)),
                target_period_end: Some(NaiveDate::from_ymd_opt(2099, 3, 31)
                    .unwrap_or(NaiveDate::MIN)),
                target_period_type: Some("month".to_owned()),
                publication_evidence: Some(
                    "Census release PDF states publication date 2099-04-08 for period 2099-03-01 through 2099-03-31".to_owned(),
                ),
                publication_evidence_url: Some(
                    "https://example.invalid/retail/report.pdf".to_owned(),
                ),
            }),
        );
    }

    #[test]
    fn records_with_the_same_release_date_keep_every_verified_period() {
        let target = MacroReferenceTarget {
            event_source: "fred".to_owned(),
            event_external_id: "9:2099-04-08".to_owned(),
            event_title: "Synthetic retail release event".to_owned(),
            same_day_event_count: 1,
        };
        let periods = [
            (
                "Synthetic retail report for March 2099",
                "https://example.invalid/retail/march-report.pdf",
                NaiveDate::from_ymd_opt(2099, 3, 1).unwrap_or(NaiveDate::MIN),
                NaiveDate::from_ymd_opt(2099, 3, 31).unwrap_or(NaiveDate::MIN),
            ),
            (
                "Synthetic retail revision for February 2099",
                "https://example.invalid/retail/february-report.pdf",
                NaiveDate::from_ymd_opt(2099, 2, 1).unwrap_or(NaiveDate::MIN),
                NaiveDate::from_ymd_opt(2099, 2, 28).unwrap_or(NaiveDate::MIN),
            ),
        ];
        let actual = periods
            .iter()
            .filter_map(|(title, url, period_start, period_end)| {
                let release_date = NaiveDate::from_ymd_opt(2099, 4, 8)?;
                census_document(
                    &target,
                    &CensusArchiveLink {
                        title: (*title).to_owned(),
                        period_start: Some(*period_start),
                        url: (*url).to_owned(),
                    },
                    &CensusPdfPublication {
                        period_start: *period_start,
                        period_end: *period_end,
                        release_date,
                    },
                )
            })
            .collect::<Vec<_>>();

        assert_eq!(
            actual,
            vec![
                MacroOfficialReference {
                    target: target.clone(),
                    kind: CalendarEventReferenceKind::IndividualDocument,
                    source: "census".to_owned(),
                    external_id: "https://example.invalid/retail/march-report.pdf".to_owned(),
                    title: "Synthetic retail report for March 2099".to_owned(),
                    url: Some("https://example.invalid/retail/march-report.pdf".to_owned()),
                    target_period_start: Some(NaiveDate::from_ymd_opt(2099, 3, 1).unwrap_or(NaiveDate::MIN)),
                    target_period_end: Some(NaiveDate::from_ymd_opt(2099, 3, 31).unwrap_or(NaiveDate::MIN)),
                    target_period_type: Some("month".to_owned()),
                    publication_evidence: Some("Census release PDF states publication date 2099-04-08 for period 2099-03-01 through 2099-03-31".to_owned()),
                    publication_evidence_url: Some("https://example.invalid/retail/march-report.pdf".to_owned()),
                },
                MacroOfficialReference {
                    target,
                    kind: CalendarEventReferenceKind::IndividualDocument,
                    source: "census".to_owned(),
                    external_id: "https://example.invalid/retail/february-report.pdf".to_owned(),
                    title: "Synthetic retail revision for February 2099".to_owned(),
                    url: Some("https://example.invalid/retail/february-report.pdf".to_owned()),
                    target_period_start: Some(NaiveDate::from_ymd_opt(2099, 2, 1).unwrap_or(NaiveDate::MIN)),
                    target_period_end: Some(NaiveDate::from_ymd_opt(2099, 2, 28).unwrap_or(NaiveDate::MIN)),
                    target_period_type: Some("month".to_owned()),
                    publication_evidence: Some("Census release PDF states publication date 2099-04-08 for period 2099-02-01 through 2099-02-28".to_owned()),
                    publication_evidence_url: Some("https://example.invalid/retail/february-report.pdf".to_owned()),
                },
            ],
        );
    }
}
