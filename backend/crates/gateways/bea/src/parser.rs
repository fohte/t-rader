use chrono::{DateTime, FixedOffset, NaiveDate};
use core_application::{
    calendar::macro_reference_targets::MacroReferenceTarget,
    macro_reference_source::MacroOfficialReference,
};
use core_domain::calendar_event::CalendarEventReferenceKind;
use reqwest::Url;
use scraper::{Html, Selector};

pub(super) struct OfficialReleaseRow {
    pub published_at: DateTime<FixedOffset>,
    pub title: String,
    pub url: Option<String>,
}

pub(super) fn parse_bea_rows(html: &str, base_url: &str) -> Option<Vec<OfficialReleaseRow>> {
    let document = Html::parse_document(html);
    let row_selector = Selector::parse("tr.release-row").ok()?;
    let time_selector = Selector::parse("time[datetime]").ok()?;
    let link_selector = Selector::parse("a[href]").ok()?;
    let base = Url::parse(base_url).ok()?;
    Some(
        document
            .select(&row_selector)
            .filter_map(|row| {
                let datetime = row
                    .select(&time_selector)
                    .next()?
                    .value()
                    .attr("datetime")?;
                let published_at = DateTime::parse_from_rfc3339(datetime).ok()?;
                let link = row.select(&link_selector).next();
                let title = link
                    .map(|link| link.text().collect::<String>().trim().to_owned())
                    .unwrap_or_default();
                let url = link
                    .and_then(|link| link.value().attr("href"))
                    .and_then(|href| resolve_official_url(&base, href));
                Some(OfficialReleaseRow {
                    published_at,
                    title,
                    url,
                })
            })
            .collect(),
    )
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

pub(super) fn bea_document(
    target: &MacroReferenceTarget,
    release_id: &str,
    release_date: NaiveDate,
    row: &OfficialReleaseRow,
    archive_url: &str,
) -> Option<MacroOfficialReference> {
    if row.published_at.date_naive() != release_date {
        return None;
    }
    let url = row.url.as_ref()?;
    let (period_start, period_end, period_type) = match release_id {
        "53" => parse_quarter(&row.title)?,
        "54" => parse_month(&row.title)?,
        _ => return None,
    };
    Some(MacroOfficialReference {
        target: target.clone(),
        kind: CalendarEventReferenceKind::IndividualDocument,
        source: "bea".to_owned(),
        external_id: url.clone(),
        title: row.title.clone(),
        url: Some(url.clone()),
        target_period_start: Some(period_start),
        target_period_end: Some(period_end),
        target_period_type: Some(period_type),
        publication_evidence: Some(format!(
            "BEA archive publication timestamp {}",
            row.published_at.to_rfc3339()
        )),
        publication_evidence_url: Some(archive_url.to_owned()),
    })
}

pub(super) fn bea_listing_url(release_id: &str) -> Option<String> {
    let (product_id, title) = match release_id {
        "53" => ("451", "GDP"),
        "54" => ("476", ""),
        _ => return None,
    };
    Some(format!(
        "https://www.bea.gov/news/archive?field_related_product_target_id={product_id}&created_1=All&title={title}"
    ))
}

fn parse_quarter(title: &str) -> Option<(NaiveDate, NaiveDate, String)> {
    let lowercase = title.to_ascii_lowercase();
    let quarters = [
        ("1st quarter", 1_u32),
        ("first quarter", 1),
        ("2nd quarter", 2),
        ("second quarter", 2),
        ("3rd quarter", 3),
        ("third quarter", 3),
        ("4th quarter", 4),
        ("fourth quarter", 4),
    ];
    quarters.iter().find_map(|(phrase, quarter)| {
        let index = lowercase.find(phrase)?;
        let year = lowercase[index + phrase.len()..]
            .split(|character: char| !character.is_ascii_digit())
            .find(|part| part.len() == 4)?
            .parse::<i32>()
            .ok()?;
        let start_month = (*quarter - 1) * 3 + 1;
        let end_month = start_month + 2;
        Some((
            NaiveDate::from_ymd_opt(year, start_month, 1)?,
            last_day_of_month(year, end_month)?,
            "quarter".to_owned(),
        ))
    })
}

fn parse_month(title: &str) -> Option<(NaiveDate, NaiveDate, String)> {
    let lowercase = title.to_ascii_lowercase();
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
        let month_position = lowercase.find(month)?;
        let year = lowercase[month_position + month.len()..]
            .split(|character: char| !character.is_ascii_digit())
            .find(|part| part.len() == 4)?
            .parse::<i32>()
            .ok()?;
        let month_number = u32::try_from(index + 1).ok()?;
        Some((
            NaiveDate::from_ymd_opt(year, month_number, 1)?,
            last_day_of_month(year, month_number)?,
            "month".to_owned(),
        ))
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

pub(super) fn parse_fred_external_id(external_id: &str) -> Option<(&str, NaiveDate)> {
    let (release_id, release_date) = external_id.split_once(':')?;
    if !matches!(release_id, "53" | "54") || release_date.len() != 10 {
        return None;
    }
    Some((
        release_id,
        NaiveDate::parse_from_str(release_date, "%Y-%m-%d").ok()?,
    ))
}

#[cfg(test)]
mod tests {
    use chrono::{DateTime, NaiveDate};
    use core_application::{
        calendar::macro_reference_targets::MacroReferenceTarget,
        macro_reference_source::MacroOfficialReference,
    };
    use core_domain::calendar_event::CalendarEventReferenceKind;
    use indoc::indoc;
    use rstest::rstest;

    use super::{OfficialReleaseRow, bea_document, parse_bea_rows, parse_fred_external_id};

    #[rstest]
    #[case::gross_domestic_product(
        "53",
        "2099-04-08",
        "Synthetic GDP release, 1st Quarter 2099",
        "2099-01-01",
        "2099-03-31",
        "quarter"
    )]
    #[case::personal_consumption(
        "54",
        "2099-04-29",
        "Synthetic Personal Income and Outlays, March 2099",
        "2099-03-01",
        "2099-03-31",
        "month"
    )]
    fn bea_records_match_the_et_date_and_explicit_target_period(
        #[case] release_id: &str,
        #[case] release_date: &str,
        #[case] title: &str,
        #[case] period_start: &str,
        #[case] period_end: &str,
        #[case] period_type: &str,
    ) {
        let archive = indoc! {
            r#"
            <table><tbody><tr class="release-row">
              <td><a href="/news/2099/release-item">TITLE</a></td>
              <td><time datetime="RELEASE_TIME">Published</time></td>
            </tr></tbody></table>
            "#
        }
        .replace("TITLE", title)
        .replace("RELEASE_TIME", &format!("{release_date}T08:30:00-04:00"));
        let target = MacroReferenceTarget {
            event_source: "fred".to_owned(),
            event_external_id: format!("{release_id}:{release_date}"),
            event_title: "Synthetic release event".to_owned(),
            same_day_event_count: 1,
        };
        let archive_url = "https://example.invalid/news/archive";
        let actual = parse_bea_rows(&archive, archive_url)
            .and_then(|rows| rows.into_iter().next())
            .and_then(|row| {
                bea_document(
                    &target,
                    release_id,
                    NaiveDate::parse_from_str(release_date, "%Y-%m-%d").ok()?,
                    &row,
                    archive_url,
                )
            });

        assert_eq!(
            actual,
            Some(MacroOfficialReference {
                target,
                kind: CalendarEventReferenceKind::IndividualDocument,
                source: "bea".to_owned(),
                external_id: "https://example.invalid/news/2099/release-item".to_owned(),
                title: title.to_owned(),
                url: Some("https://example.invalid/news/2099/release-item".to_owned()),
                target_period_start: NaiveDate::parse_from_str(period_start, "%Y-%m-%d").ok(),
                target_period_end: NaiveDate::parse_from_str(period_end, "%Y-%m-%d").ok(),
                target_period_type: Some(period_type.to_owned()),
                publication_evidence: Some(format!(
                    "BEA archive publication timestamp {release_date}T08:30:00-04:00"
                )),
                publication_evidence_url: Some(archive_url.to_owned()),
            }),
        );
    }

    #[test]
    fn bea_archive_keeps_all_qualifying_records_published_same_day() {
        let archive = indoc! {
            r#"
            <table><tbody>
              <tr class="release-row"><td><a href="/release/first">Synthetic GDP release, 1st Quarter 2099</a></td><td><time datetime="2099-04-08T08:30:00-04:00">Published</time></td></tr>
              <tr class="release-row"><td><a href="/release/second">Synthetic GDP release, 4th Quarter 2098</a></td><td><time datetime="2099-04-08T08:30:00-04:00">Published</time></td></tr>
            </tbody></table>
            "#
        };
        let target = MacroReferenceTarget {
            event_source: "fred".to_owned(),
            event_external_id: "53:2099-04-08".to_owned(),
            event_title: "Synthetic release event".to_owned(),
            same_day_event_count: 1,
        };
        let archive_url = "https://example.invalid/news/archive";
        let actual = parse_bea_rows(archive, archive_url)
            .into_iter()
            .flatten()
            .filter(|row| {
                row.published_at.date_naive()
                    == NaiveDate::from_ymd_opt(2099, 4, 8).unwrap_or(NaiveDate::MIN)
            })
            .filter_map(|row| {
                bea_document(
                    &target,
                    "53",
                    NaiveDate::from_ymd_opt(2099, 4, 8)?,
                    &row,
                    archive_url,
                )
            })
            .collect::<Vec<_>>();

        assert_eq!(
            actual,
            vec![
                MacroOfficialReference {
                    target: target.clone(),
                    kind: CalendarEventReferenceKind::IndividualDocument,
                    source: "bea".to_owned(),
                    external_id: "https://example.invalid/release/first".to_owned(),
                    title: "Synthetic GDP release, 1st Quarter 2099".to_owned(),
                    url: Some("https://example.invalid/release/first".to_owned()),
                    target_period_start: NaiveDate::from_ymd_opt(2099, 1, 1),
                    target_period_end: NaiveDate::from_ymd_opt(2099, 3, 31),
                    target_period_type: Some("quarter".to_owned()),
                    publication_evidence: Some(
                        "BEA archive publication timestamp 2099-04-08T08:30:00-04:00".to_owned()
                    ),
                    publication_evidence_url: Some(archive_url.to_owned()),
                },
                MacroOfficialReference {
                    target,
                    kind: CalendarEventReferenceKind::IndividualDocument,
                    source: "bea".to_owned(),
                    external_id: "https://example.invalid/release/second".to_owned(),
                    title: "Synthetic GDP release, 4th Quarter 2098".to_owned(),
                    url: Some("https://example.invalid/release/second".to_owned()),
                    target_period_start: NaiveDate::from_ymd_opt(2098, 10, 1),
                    target_period_end: NaiveDate::from_ymd_opt(2098, 12, 31),
                    target_period_type: Some("quarter".to_owned()),
                    publication_evidence: Some(
                        "BEA archive publication timestamp 2099-04-08T08:30:00-04:00".to_owned()
                    ),
                    publication_evidence_url: Some(archive_url.to_owned()),
                },
            ],
        );
    }

    #[test]
    fn bea_archive_keeps_rows_but_rejects_untrusted_document_urls() {
        let archive = indoc! {
            r#"
            <table><tbody>
              <tr class="release-row"><td><a href="https://attacker.invalid/release/sample.pdf">Synthetic release</a></td><td><time datetime="2099-04-08T08:30:00-04:00">Published</time></td></tr>
              <tr class="release-row"><td><a href="https://attacker.invalid@example.invalid/release/sample.pdf">Synthetic release with credentials</a></td><td><time datetime="2099-04-08T08:30:00-04:00">Published</time></td></tr>
            </tbody></table>
            "#
        };
        let actual = parse_bea_rows(archive, "https://example.invalid/news/archive").map(|rows| {
            rows.into_iter()
                .map(|row| (row.published_at.to_rfc3339(), row.title, row.url))
                .collect::<Vec<_>>()
        });

        assert_eq!(
            actual,
            Some(vec![
                (
                    "2099-04-08T08:30:00-04:00".to_owned(),
                    "Synthetic release".to_owned(),
                    None,
                ),
                (
                    "2099-04-08T08:30:00-04:00".to_owned(),
                    "Synthetic release with credentials".to_owned(),
                    None,
                ),
            ]),
        );
    }

    #[test]
    fn release_date_uses_the_archive_timezone_across_a_utc_day_boundary() {
        let target = MacroReferenceTarget {
            event_source: "fred".to_owned(),
            event_external_id: "53:2099-04-08".to_owned(),
            event_title: "Synthetic release event".to_owned(),
            same_day_event_count: 1,
        };
        let archive_url = "https://example.invalid/news/archive";
        let published_at = DateTime::parse_from_rfc3339("2099-04-08T23:30:00-04:00").ok();
        let actual = published_at.and_then(|published_at| {
            let row = OfficialReleaseRow {
                published_at,
                title: "Synthetic GDP release, 1st Quarter 2099".to_owned(),
                url: Some("https://example.invalid/news/release-item".to_owned()),
            };
            bea_document(
                &target,
                "53",
                NaiveDate::from_ymd_opt(2099, 4, 8)?,
                &row,
                archive_url,
            )
        });

        assert_eq!(
            actual,
            Some(MacroOfficialReference {
                target,
                kind: CalendarEventReferenceKind::IndividualDocument,
                source: "bea".to_owned(),
                external_id: "https://example.invalid/news/release-item".to_owned(),
                title: "Synthetic GDP release, 1st Quarter 2099".to_owned(),
                url: Some("https://example.invalid/news/release-item".to_owned()),
                target_period_start: NaiveDate::from_ymd_opt(2099, 1, 1),
                target_period_end: NaiveDate::from_ymd_opt(2099, 3, 31),
                target_period_type: Some("quarter".to_owned()),
                publication_evidence: Some(
                    "BEA archive publication timestamp 2099-04-08T23:30:00-04:00".to_owned(),
                ),
                publication_evidence_url: Some(archive_url.to_owned()),
            }),
        );
    }

    #[rstest]
    #[case::gross_domestic_product("53:2099-04-08", Some(("53", "2099-04-08")))]
    #[case::personal_consumption("54:2099-04-08", Some(("54", "2099-04-08")))]
    #[case::unsupported("88:2099-04-08", None)]
    fn fred_external_id_uses_the_et_release_date(
        #[case] input: &str,
        #[case] expected: Option<(&str, &str)>,
    ) {
        let actual = parse_fred_external_id(input)
            .map(|(release_id, date)| (release_id, date.format("%Y-%m-%d").to_string()));
        let expected = expected.map(|(release_id, date)| (release_id, date.to_owned()));

        assert_eq!(actual, expected);
    }

    #[rstest]
    #[case::gross_domestic_product(
        "53",
        "https://www.bea.gov/news/archive?field_related_product_target_id=451&created_1=All&title=GDP"
    )]
    #[case::personal_consumption(
        "54",
        "https://www.bea.gov/news/archive?field_related_product_target_id=476&created_1=All&title="
    )]
    fn archive_url_uses_the_official_product_filter(
        #[case] release_id: &str,
        #[case] expected: &str,
    ) {
        assert_eq!(
            super::bea_listing_url(release_id).as_deref(),
            Some(expected)
        );
    }
}
