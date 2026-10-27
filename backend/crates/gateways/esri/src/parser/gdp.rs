use chrono::NaiveDate;
use core_application::{
    calendar::macro_reference_targets::MacroReferenceTarget,
    macro_reference_source::MacroOfficialReference,
};
use core_domain::calendar_event::CalendarEventReferenceKind;
use reqwest::Url;
use scraper::{Html, Selector};

use super::common::{
    element_text, last_day_of_month, leading_number, parse_japanese_date, resolve_document_url,
    target_publication_date, to_gregorian_year, trailing_number,
};

pub(crate) fn gdp_archive_url(year: i32) -> String {
    format!(
        "https://www.esri.cao.go.jp/jp/sna/data/data_list/sokuhou/files/{year}/toukei_{year}.html"
    )
}

pub(crate) struct GdpArchiveParse {
    pub matching_release_count: usize,
    pub references: Vec<MacroOfficialReference>,
}

pub(crate) fn parse_gdp_archive(
    html: &str,
    archive_url: &str,
    target: &MacroReferenceTarget,
    expected_stat_code: &str,
) -> Option<GdpArchiveParse> {
    let publication_date = target_publication_date(target, expected_stat_code)?;
    let document = Html::parse_document(html);
    let row_selector = Selector::parse("tr").ok()?;
    let cell_selector = Selector::parse("td").ok()?;
    let link_selector = Selector::parse("a[href]").ok()?;
    let base = Url::parse(archive_url).ok()?;
    let mut references = Vec::new();
    let mut matching_release_count = 0;

    for row in document.select(&row_selector) {
        let cells = row.select(&cell_selector).collect::<Vec<_>>();
        let Some(publication_cell) = cells.first() else {
            continue;
        };
        let row_date = parse_japanese_date(&element_text(publication_cell))?;
        if row_date != publication_date {
            continue;
        }
        matching_release_count += 1;

        let Some(period_cell) = cells.get(1) else {
            continue;
        };
        let period_text = element_text(period_cell);
        let Some((period_start, period_end)) = parse_gdp_quarter(&period_text) else {
            continue;
        };
        let document_candidates = row
            .select(&link_selector)
            .filter_map(|link| {
                let title = element_text(&link);
                title.contains("結果の概要").then(|| {
                    link.value()
                        .attr("href")
                        .and_then(|href| resolve_document_url(&base, href))
                })
            })
            .collect::<Vec<_>>();
        let document_url = match document_candidates.as_slice() {
            [Some(url)] => url.clone(),
            _ => continue,
        };

        references.push(MacroOfficialReference {
            target: target.clone(),
            kind: CalendarEventReferenceKind::IndividualDocument,
            source: "esri_jp".to_owned(),
            external_id: document_url.clone(),
            title: period_text.clone(),
            url: Some(document_url),
            target_period_start: Some(period_start),
            target_period_end: Some(period_end),
            target_period_type: Some("quarter".to_owned()),
            publication_evidence: Some(format!(
                "Official GDP archive row states publication date {row_date} for {period_text}"
            )),
            publication_evidence_url: Some(archive_url.to_owned()),
        });
    }

    Some(GdpArchiveParse {
        matching_release_count,
        references,
    })
}

fn parse_gdp_quarter(value: &str) -> Option<(NaiveDate, NaiveDate)> {
    value
        .match_indices('年')
        .filter_map(|(position, _)| {
            let year_value = trailing_number(&value[..position])?;
            let year = to_gregorian_year(year_value, &value[..position])?;
            let remainder = &value[position + '年'.len_utf8()..];
            let (start_month, remainder) = leading_number(remainder)?;
            let separator_length = remainder.chars().next()?.len_utf8();
            if !matches!(
                remainder.chars().next()?,
                '-' | '－' | '−' | '～' | '〜' | '―' | '–'
            ) {
                return None;
            }
            let (end_month, remainder) = leading_number(&remainder[separator_length..])?;
            if !remainder.starts_with("月期")
                || !(1..=10).contains(&start_month)
                || end_month != start_month + 2
                || (start_month - 1) % 3 != 0
            {
                return None;
            }
            let period_start = NaiveDate::from_ymd_opt(year, start_month, 1)?;
            let period_end = last_day_of_month(year, end_month)?;
            Some((period_start, period_end))
        })
        .next_back()
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;
    use core_application::{
        calendar::macro_reference_targets::MacroReferenceTarget,
        macro_reference_source::MacroOfficialReference,
    };
    use core_domain::calendar_event::CalendarEventReferenceKind;
    use indoc::indoc;

    use super::*;

    fn target(code: &str, published_at: &str) -> MacroReferenceTarget {
        MacroReferenceTarget {
            event_source: "e_stat".to_owned(),
            event_external_id: format!("{code}:{published_at}"),
            event_title: "架空の統計イベント".to_owned(),
            same_day_event_count: 1,
        }
    }

    #[test]
    fn gdp_archive_keeps_each_document_for_same_publication_day() {
        let archive = indoc! {
            r#"
            <table><tbody>
              <tr><td>2099年4月8日</td><td>2099年1-3月期2次推計値</td><td><a href="/result/sample-a.pdf">結果の概要</a></td></tr>
              <tr><td>2099年4月8日</td><td>2098年10-12月期確報値</td><td><a href="/result/sample-b.pdf">結果の概要</a></td></tr>
            </tbody></table>
            "#
        };
        let archive_url = "https://example.invalid/gdp/2099.html";
        let target = target("sample-stat-code", "209904080850");

        let actual = parse_gdp_archive(archive, archive_url, &target, "sample-stat-code");

        assert_eq!(
            actual.map(|parsed| (parsed.matching_release_count, parsed.references)),
            Some((2, vec![
                MacroOfficialReference {
                    target: target.clone(),
                    kind: CalendarEventReferenceKind::IndividualDocument,
                    source: "esri_jp".to_owned(),
                    external_id: "https://example.invalid/result/sample-a.pdf".to_owned(),
                    title: "2099年1-3月期2次推計値".to_owned(),
                    url: Some("https://example.invalid/result/sample-a.pdf".to_owned()),
                    target_period_start: NaiveDate::from_ymd_opt(2099, 1, 1),
                    target_period_end: NaiveDate::from_ymd_opt(2099, 3, 31),
                    target_period_type: Some("quarter".to_owned()),
                    publication_evidence: Some("Official GDP archive row states publication date 2099-04-08 for 2099年1-3月期2次推計値".to_owned()),
                    publication_evidence_url: Some(archive_url.to_owned()),
                },
                MacroOfficialReference {
                    target,
                    kind: CalendarEventReferenceKind::IndividualDocument,
                    source: "esri_jp".to_owned(),
                    external_id: "https://example.invalid/result/sample-b.pdf".to_owned(),
                    title: "2098年10-12月期確報値".to_owned(),
                    url: Some("https://example.invalid/result/sample-b.pdf".to_owned()),
                    target_period_start: NaiveDate::from_ymd_opt(2098, 10, 1),
                    target_period_end: NaiveDate::from_ymd_opt(2098, 12, 31),
                    target_period_type: Some("quarter".to_owned()),
                    publication_evidence: Some("Official GDP archive row states publication date 2099-04-08 for 2098年10-12月期確報値".to_owned()),
                    publication_evidence_url: Some(archive_url.to_owned()),
                },
            ])),
        );
    }

    #[test]
    fn gdp_archive_keeps_matching_rows_when_document_urls_are_untrusted() {
        let archive = indoc! {
            r#"
            <table><tbody>
              <tr><td>2099年4月8日</td><td>2099年1-3月期2次推計値</td><td><a href="/result/allowed.pdf">結果の概要</a></td></tr>
              <tr><td>2099年4月8日</td><td>2098年10-12月期確報値</td><td><a href="https://attacker.invalid/result/external.pdf">結果の概要</a></td></tr>
              <tr><td>2099年4月8日</td><td>2098年7-9月期確報値</td><td><a href="https://attacker.invalid@example.invalid/result/credentialed.pdf">結果の概要</a></td></tr>
            </tbody></table>
            "#
        };
        let target = target("sample-stat-code", "209904080850");
        let archive_url = "https://example.invalid/gdp/2099.html";

        let actual = parse_gdp_archive(archive, archive_url, &target, "sample-stat-code");

        assert_eq!(
            actual.map(|parsed| (parsed.matching_release_count, parsed.references)),
            Some((
                3,
                vec![MacroOfficialReference {
                    target,
                    kind: CalendarEventReferenceKind::IndividualDocument,
                    source: "esri_jp".to_owned(),
                    external_id: "https://example.invalid/result/allowed.pdf".to_owned(),
                    title: "2099年1-3月期2次推計値".to_owned(),
                    url: Some("https://example.invalid/result/allowed.pdf".to_owned()),
                    target_period_start: NaiveDate::from_ymd_opt(2099, 1, 1),
                    target_period_end: NaiveDate::from_ymd_opt(2099, 3, 31),
                    target_period_type: Some("quarter".to_owned()),
                    publication_evidence: Some("Official GDP archive row states publication date 2099-04-08 for 2099年1-3月期2次推計値".to_owned()),
                    publication_evidence_url: Some(archive_url.to_owned()),
                }],
            )),
        );
    }

    #[test]
    fn gdp_archive_url_uses_the_release_year() {
        assert_eq!(
            gdp_archive_url(2099),
            "https://www.esri.cao.go.jp/jp/sna/data/data_list/sokuhou/files/2099/toukei_2099.html"
        );
    }
}
