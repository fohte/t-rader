use std::collections::HashMap;

use chrono::NaiveDate;
use scraper::{ElementRef, Html, Selector};

use crate::official_http::OfficialHttpClient;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct DecisionEntry {
    pub publication_date: NaiveDate,
    pub title: String,
    pub url: String,
    pub archive_url: String,
}

pub(super) fn decision_archive_pages(
    html: &str,
    page_url: &str,
) -> Result<HashMap<i32, String>, String> {
    let document = Html::parse_document(html);
    let lists = selector("dl[data-snippets]")?;
    let mut pages = HashMap::new();
    for list in document.select(&lists) {
        let Some(snippets) = list.value().attr("data-snippets") else {
            continue;
        };
        for snippet in snippets
            .split(',')
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            let Some(year) = snippet.split('/').find_map(|part| part.parse::<i32>().ok()) else {
                continue;
            };
            let Some(url) = OfficialHttpClient::resolve_url(page_url, snippet) else {
                continue;
            };
            pages.entry(year).or_insert(url);
        }
    }
    if pages.is_empty() {
        return Err("ECB monetary policy decision archive year links were not found".to_owned());
    }
    Ok(pages)
}

pub(super) fn decision_entries(html: &str, page_url: &str) -> Result<Vec<DecisionEntry>, String> {
    let document = Html::parse_document(html);
    let date_selector = selector("dt")?;
    let release_selector = selector("dd")?;
    let links = selector("a[href]")?;
    let dates = document.select(&date_selector).collect::<Vec<_>>();
    let releases = document.select(&release_selector).collect::<Vec<_>>();
    if dates.is_empty() && releases.is_empty() {
        return Err("ECB monetary policy decision archive list was not found".to_owned());
    }
    if dates.len() != releases.len() {
        return Err("ECB decision archive entries were incomplete".to_owned());
    }

    let mut entries = Vec::new();
    for (date, release) in dates.into_iter().zip(releases) {
        let date_text = element_text(&date);
        let publication_date = NaiveDate::parse_from_str(date_text.trim(), "%d/%m/%Y")
            .map_err(|error| format!("invalid ECB decision archive date: {error}"))?;
        for link in release.select(&links) {
            let title = element_text(&link);
            if !is_monetary_policy_decision(&title) {
                continue;
            }
            let Some(url) = link
                .value()
                .attr("href")
                .and_then(|href| OfficialHttpClient::resolve_url(page_url, href))
            else {
                continue;
            };
            entries.push(DecisionEntry {
                publication_date,
                title,
                url,
                archive_url: page_url.to_owned(),
            });
        }
    }
    Ok(entries)
}

fn is_monetary_policy_decision(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "monetary policy decisions" | "combined monetary policy decisions and statement"
    )
}

fn element_text(element: &ElementRef<'_>) -> String {
    element
        .text()
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn selector(value: &str) -> Result<Selector, String> {
    Selector::parse(value).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;

    use super::{DecisionEntry, decision_archive_pages, decision_entries};

    const ORIGIN: &str = "https://www.ecb.europa.eu";
    const INDEX_URL: &str = "https://www.ecb.europa.eu/press/govcdec/mopo/html/index.en.html";
    const ARCHIVE_URL: &str =
        "https://www.ecb.europa.eu/press/govcdec/mopo/previous/html/index.en.html";
    const INDEX_FIXTURE: &str = include_str!("../fixtures/decision_archive_index_synthetic.html");
    const ARCHIVE_FIXTURE: &str = include_str!("../fixtures/decision_archive_synthetic.html");
    const AMBIGUOUS_FIXTURE: &str =
        include_str!("../fixtures/decision_archive_ambiguous_synthetic.html");

    #[test]
    fn decision_archive_links_only_the_exact_date_and_official_document_titles() {
        assert_eq!(
            decision_entries(ARCHIVE_FIXTURE, ARCHIVE_URL).expect("parse archive"),
            vec![DecisionEntry {
                publication_date: NaiveDate::from_ymd_opt(2099, 4, 8).unwrap_or(NaiveDate::MIN),
                title: "Monetary policy decisions".to_owned(),
                url: format!("{ORIGIN}/press/fictional/decision-document.html"),
                archive_url: ARCHIVE_URL.to_owned(),
            }],
        );
    }

    #[test]
    fn ambiguous_decisions_on_a_date_remain_distinct_candidates() {
        assert_eq!(
            decision_entries(AMBIGUOUS_FIXTURE, ARCHIVE_URL)
                .expect("parse archive")
                .into_iter()
                .map(|entry| entry.url)
                .collect::<Vec<_>>(),
            vec![
                format!("{ORIGIN}/press/fictional/decision-one.html"),
                format!("{ORIGIN}/press/fictional/decision-two.html"),
            ],
        );
    }

    #[test]
    fn links_outside_the_official_https_origin_are_discarded() {
        let html = r#"<div class="definition-list"><dl>
          <dt>08/04/2099</dt>
          <dd><a href="https://www.example.invalid/release.html">Monetary policy decisions</a></dd>
        </dl></div>"#;

        assert_eq!(decision_entries(html, ARCHIVE_URL), Ok(Vec::new()));
    }

    #[test]
    fn archive_pages_are_selected_from_the_official_lazy_archive_index() {
        assert_eq!(
            decision_archive_pages(INDEX_FIXTURE, INDEX_URL).expect("archive pages"),
            [(
                2099,
                format!("{ORIGIN}/press/govcdec/mopo/2099/html/index_include.en.html")
            )]
            .into_iter()
            .collect(),
        );
    }
}
