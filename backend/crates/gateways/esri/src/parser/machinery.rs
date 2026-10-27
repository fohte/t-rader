use chrono::NaiveDate;
use core_application::{
    calendar::macro_reference_targets::MacroReferenceTarget,
    macro_reference_source::MacroOfficialReference,
};
use core_domain::calendar_event::CalendarEventReferenceKind;
use reqwest::Url;
use scraper::{Html, Selector};
use std::collections::HashSet;

use super::common::{
    element_text, last_day_of_month, leading_number, parse_japanese_date, resolve_document_url,
    target_publication_date, to_gregorian_year, trailing_number,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MachineryReleasePage {
    pub title: String,
    pub url: String,
    pub period_start: NaiveDate,
}

pub(crate) fn parse_machinery_index(
    html: &str,
    index_url: &str,
) -> Option<Vec<MachineryReleasePage>> {
    let document = Html::parse_document(html);
    let link_selector = Selector::parse("a[href]").ok()?;
    let base = Url::parse(index_url).ok()?;
    let mut seen = HashSet::new();
    Some(
        document
            .select(&link_selector)
            .filter_map(|link| {
                let href = link.value().attr("href")?;
                let title = element_text(&link);
                if !href.contains("juchu.html")
                    || href.ends_with("99juchu.html")
                    || !title.contains("実績")
                {
                    return None;
                }
                let (year, month) = parse_year_month(&title)?;
                let period_start = NaiveDate::from_ymd_opt(year, month, 1)?;
                let url = resolve_document_url(&base, href)?;
                seen.insert(url.clone()).then_some(MachineryReleasePage {
                    title,
                    url,
                    period_start,
                })
            })
            .collect(),
    )
}

pub(crate) fn parse_machinery_release_page(
    html: &str,
    page_url: &str,
    target: &MacroReferenceTarget,
    expected_stat_code: &str,
) -> Option<MacroOfficialReference> {
    let publication_date = target_publication_date(target, expected_stat_code)?;
    let document = Html::parse_document(html);
    let heading_selector = Selector::parse("h1").ok()?;
    let title = document
        .select(&heading_selector)
        .next()
        .map(|heading| element_text(&heading))?;
    let actual_publication_date = parse_machinery_publication_date(html)?;
    if actual_publication_date != publication_date {
        return None;
    }
    let (period_start, period_end) = parse_machinery_period(&title)?;
    let canonical_url = Url::parse(page_url).ok()?.to_string();

    Some(MacroOfficialReference {
        target: target.clone(),
        kind: CalendarEventReferenceKind::IndividualDocument,
        source: "esri_jp".to_owned(),
        external_id: canonical_url.clone(),
        title: title.clone(),
        url: Some(canonical_url),
        target_period_start: Some(period_start),
        target_period_end: Some(period_end),
        target_period_type: Some("month".to_owned()),
        publication_evidence: Some(format!(
            "Official machinery release page states publication date {actual_publication_date} for {title}"
        )),
        publication_evidence_url: Some(page_url.to_owned()),
    })
}

pub(crate) fn parse_machinery_publication_date(html: &str) -> Option<NaiveDate> {
    let document = Html::parse_document(html);
    let body_selector = Selector::parse("body").ok()?;
    document
        .select(&body_selector)
        .flat_map(|body| body.text())
        .flat_map(str::lines)
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .find_map(parse_japanese_date)
}
fn parse_machinery_period(value: &str) -> Option<(NaiveDate, NaiveDate)> {
    let actual_period = value.split("実績").next()?;
    let (year, month) = parse_year_month(actual_period)?;
    Some((
        NaiveDate::from_ymd_opt(year, month, 1)?,
        last_day_of_month(year, month)?,
    ))
}

fn parse_year_month(value: &str) -> Option<(i32, u32)> {
    value.match_indices('年').find_map(|(position, _)| {
        let year_value = trailing_number(&value[..position])?;
        let year = to_gregorian_year(year_value, &value[..position])?;
        let remainder = value[position + '年'.len_utf8()..].trim_start();
        let (month, remainder) = leading_number(remainder)?;
        remainder.starts_with('月').then_some((year, month))
    })
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
    fn machinery_index_exposes_monthly_release_pages() {
        let index = indoc! {
            r#"
            <main>
              <a href="2099/9910juchu.html">令和81年10月実績</a>
              <a href="2099/9911juchu.html">令和81年11月実績</a>
              <a href="99juchu.html">過去の調査報告</a>
            </main>
            "#
        };
        let index_url = "https://example.invalid/machinery/index.html";
        let actual = parse_machinery_index(index, index_url);

        assert_eq!(
            actual,
            Some(vec![
                MachineryReleasePage {
                    title: "令和81年10月実績".to_owned(),
                    url: "https://example.invalid/machinery/2099/9910juchu.html".to_owned(),
                    period_start: NaiveDate::from_ymd_opt(2099, 10, 1).unwrap_or(NaiveDate::MIN),
                },
                MachineryReleasePage {
                    title: "令和81年11月実績".to_owned(),
                    url: "https://example.invalid/machinery/2099/9911juchu.html".to_owned(),
                    period_start: NaiveDate::from_ymd_opt(2099, 11, 1).unwrap_or(NaiveDate::MIN),
                },
            ]),
        );
    }

    #[test]
    fn machinery_index_ignores_release_pages_with_untrusted_urls() {
        let index = indoc! {
            r#"
            <main>
              <a href="https://attacker.invalid/releases/sample-juchu.html">令和81年10月実績</a>
              <a href="https://attacker.invalid@example.invalid/releases/sample-juchu.html">令和81年10月実績</a>
            </main>
            "#
        };

        let actual = parse_machinery_index(index, "https://example.invalid/machinery/index.html");

        assert_eq!(actual, Some(Vec::new()));
    }

    #[test]
    fn machinery_release_page_requires_published_date_and_explicit_period() {
        let page = indoc! {
            r#"
            <main>
              <h1>令和81年10月実績 - 架空の受注結果</h1>
              <p>令和81年11月15日</p>
              <a href="/sample.pdf">概要資料</a>
            </main>
            "#
        };
        let page_url = "https://example.invalid/machinery/9910juchu.html";
        let target = target("sample-stat-code", "209911150850");

        let actual = parse_machinery_release_page(page, page_url, &target, "sample-stat-code");

        assert_eq!(
            actual,
            Some(MacroOfficialReference {
                target,
                kind: CalendarEventReferenceKind::IndividualDocument,
                source: "esri_jp".to_owned(),
                external_id: page_url.to_owned(),
                title: "令和81年10月実績 - 架空の受注結果".to_owned(),
                url: Some(page_url.to_owned()),
                target_period_start: NaiveDate::from_ymd_opt(2099, 10, 1),
                target_period_end: NaiveDate::from_ymd_opt(2099, 10, 31),
                target_period_type: Some("month".to_owned()),
                publication_evidence: Some("Official machinery release page states publication date 2099-11-15 for 令和81年10月実績 - 架空の受注結果".to_owned()),
                publication_evidence_url: Some(page_url.to_owned()),
            }),
        );
    }
}
