use std::collections::HashSet;

use chrono::NaiveDate;
use core_application::{
    calendar::macro_reference_targets::MacroReferenceTarget,
    macro_reference_source::MacroOfficialReference,
};
use core_domain::calendar_event::CalendarEventReferenceKind;
use reqwest::Url;
use scraper::{Html, Selector};

use super::common::{
    element_text, last_day_of_month, leading_number, parse_japanese_date, target_publication_date,
    to_gregorian_year, trailing_number,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct IipReleasePage {
    pub title: String,
    pub url: String,
    pub period_start: NaiveDate,
}

pub(crate) fn parse_iip_archive(html: &str, archive_url: &str) -> Option<Vec<IipReleasePage>> {
    let document = Html::parse_document(html);
    let link_selector = Selector::parse("a[href]").ok()?;
    let base = Url::parse(archive_url).ok()?;
    let mut seen = HashSet::new();
    Some(
        document
            .select(&link_selector)
            .filter_map(|link| {
                let href = link.value().attr("href")?;
                let title = element_text(&link);
                if !href.contains("result/") || !(title.contains("概要") || title.contains("結果"))
                {
                    return None;
                }
                let filename = href.rsplit('/').next()?;
                let stem = filename.strip_suffix(".html")?;
                let (_, period_code) = stem.rsplit_once('_')?;
                let period_bytes = period_code.as_bytes();
                if period_code.len() != 8
                    || !period_bytes[..4].iter().all(u8::is_ascii_digit)
                    || !period_bytes[4..6].iter().all(u8::is_ascii_digit)
                    || !matches!(std::str::from_utf8(&period_bytes[6..]).ok()?, "sj" | "kj")
                {
                    return None;
                }
                let month = std::str::from_utf8(&period_bytes[4..6])
                    .ok()?
                    .parse::<u32>()
                    .ok()?;
                if !(1..=12).contains(&month) {
                    return None;
                }
                let year = std::str::from_utf8(&period_bytes[..4])
                    .ok()?
                    .parse::<i32>()
                    .ok()?;
                let period_start = NaiveDate::from_ymd_opt(year, month, 1)?;
                let url = resolve_official_url(&base, href)?;
                if !seen.insert(url.clone()) {
                    return None;
                }
                Some(IipReleasePage {
                    title,
                    url,
                    period_start,
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

pub(crate) fn parse_iip_release_page(
    html: &str,
    page_url: &str,
    target: &MacroReferenceTarget,
    expected_stat_code: &str,
) -> Option<MacroOfficialReference> {
    let publication_date = target_publication_date(target, expected_stat_code)?;
    let document = Html::parse_document(html);
    let heading_selector = Selector::parse("h1").ok()?;
    let description_selector = Selector::parse("meta[property=\"og:description\"]").ok()?;
    let title = document
        .select(&heading_selector)
        .next()
        .map(|heading| element_text(&heading))?;
    let description = document
        .select(&description_selector)
        .next()
        .and_then(|meta| meta.value().attr("content"))
        .unwrap_or_default();
    let actual_publication_date = parse_iip_publication_date(html)?;
    if actual_publication_date != publication_date {
        return None;
    }
    let (period_start, period_end, period_text) = parse_iip_period(description)?;
    let canonical_url = Url::parse(page_url).ok()?.to_string();

    Some(MacroOfficialReference {
        target: target.clone(),
        kind: CalendarEventReferenceKind::IndividualDocument,
        source: "meti_jp".to_owned(),
        external_id: canonical_url.clone(),
        title,
        url: Some(canonical_url.clone()),
        target_period_start: Some(period_start),
        target_period_end: Some(period_end),
        target_period_type: Some("month".to_owned()),
        publication_evidence: Some(format!(
            "Official industrial production result page states publication date {actual_publication_date} for {period_text}"
        )),
        publication_evidence_url: Some(canonical_url),
    })
}

pub(crate) fn parse_iip_publication_date(html: &str) -> Option<NaiveDate> {
    let document = Html::parse_document(html);
    let body_selector = Selector::parse("body").ok()?;
    let description_selector = Selector::parse("meta[property=\"og:description\"]").ok()?;
    let description = document
        .select(&description_selector)
        .next()
        .and_then(|meta| meta.value().attr("content"))
        .unwrap_or_default();
    let page_text = document
        .select(&body_selector)
        .flat_map(|body| body.text())
        .chain(std::iter::once(description))
        .collect::<Vec<_>>()
        .join(" ");
    page_text.split('(').find_map(parse_japanese_date)
}

fn parse_iip_period(value: &str) -> Option<(NaiveDate, NaiveDate, String)> {
    value.match_indices('年').find_map(|(position, _)| {
        let year_value = trailing_number(&value[..position])?;
        let year = to_gregorian_year(year_value, &value[..position])?;
        let remainder = &value[position + '年'.len_utf8()..];
        let (month, remainder) = leading_number(remainder)?;
        let remainder = remainder.strip_prefix('月')?;
        if remainder
            .chars()
            .next()
            .is_some_and(|character| character.is_ascii_digit())
        {
            return None;
        }
        let has_period_marker = remainder.starts_with('分')
            || remainder.starts_with("速報")
            || remainder.starts_with("確報")
            || remainder
                .chars()
                .take(16)
                .collect::<String>()
                .contains("速報")
            || remainder
                .chars()
                .take(16)
                .collect::<String>()
                .contains("確報");
        if !has_period_marker {
            return None;
        }
        let start = NaiveDate::from_ymd_opt(year, month, 1)?;
        let end = last_day_of_month(year, month)?;
        Some((start, end, format!("{year}年{month}月")))
    })
}
