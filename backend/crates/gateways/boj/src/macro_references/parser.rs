use std::collections::HashMap;

use chrono::NaiveDate;
use reqwest::Url;
use scraper::{ElementRef, Html, Selector};

use crate::official_http::OfficialHttpClient;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct StatementEntry {
    pub publication_date: NaiveDate,
    pub title: String,
    pub url: String,
    pub archive_url: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct TankanEntry {
    pub publication_date: NaiveDate,
    pub title: String,
    pub survey_title: String,
    pub url: String,
    pub archive_url: String,
    pub survey_period_start: NaiveDate,
    pub survey_period_end: NaiveDate,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct TankanArchivePage {
    pub first_year: i32,
    pub last_year: i32,
    pub url: String,
}

pub(super) fn statement_archive_pages(
    html: &str,
    page_url: &str,
) -> Result<HashMap<i32, String>, String> {
    let document = Html::parse_document(html);
    let links = selector("a[href]")?;
    let mut pages = HashMap::new();
    for link in document.select(&links) {
        let label = element_text(&link);
        let Ok(year) = label.parse::<i32>() else {
            continue;
        };
        let Some(href) = link.value().attr("href") else {
            continue;
        };
        if let Some(url) = OfficialHttpClient::resolve_url(page_url, href) {
            pages.insert(year, url);
        }
    }
    if pages.is_empty() {
        return Err("BOJ statement archive year links were not found".to_owned());
    }
    Ok(pages)
}

pub(super) fn statement_entries(
    html: &str,
    page_url: &str,
    page_year: i32,
) -> Result<Vec<StatementEntry>, String> {
    let document = Html::parse_document(html);
    let rows = selector("#contents .tbl-box table tbody tr")?;
    let cells = selector("td")?;
    let links = selector("a[href]")?;
    let mut entries = Vec::new();
    for row in document.select(&rows) {
        let row_cells = row.select(&cells).collect::<Vec<_>>();
        if row_cells.len() < 2 {
            continue;
        }
        let title = element_text(&row_cells[1]);
        if !title
            .to_ascii_lowercase()
            .contains("statement on monetary policy")
        {
            continue;
        }
        let Some(link) = row_cells[1].select(&links).find(|link| {
            element_text(link)
                .to_ascii_lowercase()
                .contains("statement on monetary policy")
        }) else {
            continue;
        };
        let Some(url) = link
            .value()
            .attr("href")
            .and_then(|href| OfficialHttpClient::resolve_url(page_url, href))
        else {
            continue;
        };
        let Some(publication_date) =
            parse_english_date(&element_text(&row_cells[0]), Some(page_year))
        else {
            continue;
        };
        entries.push(StatementEntry {
            publication_date,
            title: clean_title(&title),
            url,
            archive_url: page_url.to_owned(),
        });
    }
    if document.select(&rows).next().is_none() {
        return Err("BOJ statement archive table was not found".to_owned());
    }
    Ok(entries)
}

pub(super) fn tankan_archive_pages(
    html: &str,
    page_url: &str,
) -> Result<Vec<TankanArchivePage>, String> {
    let document = Html::parse_document(html);
    let links = selector("a[href]")?;
    let mut pages = Vec::new();
    for link in document.select(&links) {
        let label = element_text(&link).to_ascii_lowercase();
        if !label.starts_with("from ") {
            continue;
        }
        let years = label
            .split(|character: char| !character.is_ascii_digit())
            .filter(|value| !value.is_empty())
            .filter_map(|value| value.parse::<i32>().ok())
            .collect::<Vec<_>>();
        let Some(first_year) = years.first().copied() else {
            continue;
        };
        let last_year = years.get(1).copied().unwrap_or(first_year);
        let Some(url) = link
            .value()
            .attr("href")
            .and_then(|href| OfficialHttpClient::resolve_url(page_url, href))
        else {
            continue;
        };
        pages.push(TankanArchivePage {
            first_year,
            last_year,
            url,
        });
    }
    if pages.is_empty() {
        return Err("BOJ Tankan archive period links were not found".to_owned());
    }
    Ok(pages)
}

pub(super) fn tankan_entries(html: &str, page_url: &str) -> Result<Vec<TankanEntry>, String> {
    let document = Html::parse_document(html);
    let rows = selector("#contents .tbl-box table tbody tr")?;
    let cells = selector("td")?;
    let links = selector("a[href]")?;
    let mut entries = Vec::new();
    for row in document.select(&rows) {
        let row_cells = row.select(&cells).collect::<Vec<_>>();
        if row_cells.len() < 3 {
            continue;
        }
        let Some(publication_date) = parse_english_date(&element_text(&row_cells[0]), None) else {
            continue;
        };
        let survey_title = element_text(&row_cells[1]);
        let Some((survey_period_start, survey_period_end)) = parse_survey_period(&survey_title)
        else {
            continue;
        };
        let pdf_links = row_cells[2]
            .select(&links)
            .filter_map(|link| {
                let href = link.value().attr("href")?;
                let url = OfficialHttpClient::resolve_url(page_url, href)?;
                let path = Url::parse(&url).ok()?.path().to_ascii_lowercase();
                path.ends_with(".pdf").then_some(url)
            })
            .collect::<Vec<_>>();
        if pdf_links.len() != 1 {
            continue;
        }
        let Some(url) = pdf_links.first() else {
            continue;
        };
        entries.push(TankanEntry {
            publication_date,
            title: format!("TANKAN (Summary) {survey_title}"),
            survey_title,
            url: url.clone(),
            archive_url: page_url.to_owned(),
            survey_period_start,
            survey_period_end,
        });
    }
    if document.select(&rows).next().is_none() {
        return Err("BOJ Tankan archive table was not found".to_owned());
    }
    Ok(entries)
}

fn parse_survey_period(value: &str) -> Option<(NaiveDate, NaiveDate)> {
    let words = value.split_whitespace().collect::<Vec<_>>();
    if words.len() < 3 || !words.last()?.eq_ignore_ascii_case("survey") {
        return None;
    }
    let month = parse_month(words[0])?;
    let year = words[1].parse::<i32>().ok()?;
    let start = NaiveDate::from_ymd_opt(year, month, 1)?;
    let (next_year, next_month) = if month == 12 {
        (year.checked_add(1)?, 1)
    } else {
        (year, month + 1)
    };
    let end = NaiveDate::from_ymd_opt(next_year, next_month, 1)?.pred_opt()?;
    Some((start, end))
}

fn parse_english_date(value: &str, fallback_year: Option<i32>) -> Option<NaiveDate> {
    let parts = value
        .split(|character: char| !character.is_ascii_alphanumeric())
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    let month_index = parts.iter().position(|part| parse_month(part).is_some())?;
    let month = parse_month(parts[month_index])?;
    let day = parts.get(month_index + 1)?.parse::<u32>().ok()?;
    let year = parts
        .get(month_index + 2)
        .and_then(|value| value.parse::<i32>().ok())
        .or(fallback_year)?;
    NaiveDate::from_ymd_opt(year, month, day)
}

fn parse_month(value: &str) -> Option<u32> {
    match value.trim_end_matches('.').to_ascii_lowercase().as_str() {
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

fn clean_title(value: &str) -> String {
    value
        .split_once("[PDF")
        .map_or(value, |(title, _)| title)
        .trim()
        .to_owned()
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

    use super::{
        StatementEntry, TankanArchivePage, TankanEntry, statement_archive_pages, statement_entries,
        tankan_archive_pages, tankan_entries,
    };

    const BOJ_ORIGIN: &str = "https://www.boj.or.jp";
    const STATEMENT_INDEX_URL: &str = "https://www.boj.or.jp/en/mopo/mpmdeci/state_all/index.htm";
    const TANKAN_INDEX_URL: &str = "https://www.boj.or.jp/en/statistics/tk/gaiyo/index.htm";
    const STATEMENT_INDEX: &str =
        include_str!("../fixtures/statement_archive_index_synthetic.html");
    const STATEMENT_ARCHIVE: &str = include_str!("../fixtures/statement_archive_synthetic.html");
    const TANKAN_INDEX: &str = include_str!("../fixtures/tankan_archive_index_synthetic.html");
    const TANKAN_ARCHIVE: &str = include_str!("../fixtures/tankan_archive_synthetic.html");
    const EXTERNAL_LINK_ARCHIVE: &str =
        include_str!("../fixtures/external_statement_link_synthetic.html");

    #[test]
    fn statement_archive_uses_year_links_and_exact_publication_date() {
        let pages =
            statement_archive_pages(STATEMENT_INDEX, STATEMENT_INDEX_URL).expect("archive links");
        let page_url = pages.get(&2099).expect("fictional archive year").clone();

        assert_eq!(
            statement_entries(STATEMENT_ARCHIVE, &page_url, 2099).expect("entries"),
            vec![StatementEntry {
                publication_date: NaiveDate::from_ymd_opt(2099, 4, 8).unwrap_or(NaiveDate::MIN),
                title: "Statement on Monetary Policy".to_owned(),
                url: format!("{BOJ_ORIGIN}/en/mopo/archive/fictional-statement.pdf"),
                archive_url: page_url,
            }],
        );
    }

    #[test]
    fn tankan_archive_matches_release_date_and_records_survey_period() {
        let archive_url =
            format!("{BOJ_ORIGIN}/en/statistics/tk/gaiyo/archive/fictional-current.html");

        assert_eq!(
            tankan_entries(TANKAN_ARCHIVE, &archive_url).expect("entries"),
            vec![TankanEntry {
                publication_date: NaiveDate::from_ymd_opt(2099, 7, 1).unwrap_or(NaiveDate::MIN),
                title: "TANKAN (Summary) June 2099 Survey".to_owned(),
                survey_title: "June 2099 Survey".to_owned(),
                url: format!("{BOJ_ORIGIN}/en/statistics/tk/archive/fictional-summary.pdf"),
                archive_url,
                survey_period_start: NaiveDate::from_ymd_opt(2099, 6, 1).unwrap_or(NaiveDate::MIN),
                survey_period_end: NaiveDate::from_ymd_opt(2099, 6, 30).unwrap_or(NaiveDate::MIN),
            }],
        );
    }

    #[test]
    fn tankan_archive_year_links_cover_their_published_periods() {
        assert_eq!(
            tankan_archive_pages(TANKAN_INDEX, TANKAN_INDEX_URL).expect("archive links"),
            vec![
                TankanArchivePage {
                    first_year: 2099,
                    last_year: 2099,
                    url: format!(
                        "{BOJ_ORIGIN}/en/statistics/tk/gaiyo/archive/fictional-current.html"
                    ),
                },
                TankanArchivePage {
                    first_year: 2094,
                    last_year: 2098,
                    url: format!(
                        "{BOJ_ORIGIN}/en/statistics/tk/gaiyo/archive/fictional-history.html"
                    ),
                },
            ],
        );
    }

    #[test]
    fn external_document_links_must_stay_on_the_official_https_origin() {
        assert_eq!(
            statement_entries(
                EXTERNAL_LINK_ARCHIVE,
                "https://www.boj.or.jp/en/mopo/archive/fictional-year-list.html",
                2099,
            ),
            Ok(Vec::new()),
        );
    }
}
