use std::collections::{HashMap, HashSet};

use chrono::NaiveDate;
use scraper::{ElementRef, Html, Selector};

use crate::{official_http::OfficialHttpClient, parser::parse_statement_date};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct StatementEntry {
    pub publication_date: NaiveDate,
    pub title: String,
    pub url: String,
    pub archive_url: String,
}

pub(super) fn calendar_statement_entries(
    html: &str,
    page_url: &str,
) -> Result<Vec<StatementEntry>, String> {
    let document = Html::parse_document(html);
    let panels = selector("#article .panel.panel-default")?;
    let headings = selector(".panel-heading h4")?;
    let meetings = selector(".fomc-meeting")?;
    let months = selector(".fomc-meeting__month")?;
    let dates = selector(".fomc-meeting__date")?;
    let statement_columns = selector(".col-lg-2")?;
    let labels = selector("strong")?;
    let links = selector("a[href]")?;
    let panel_nodes = document.select(&panels).collect::<Vec<_>>();
    if panel_nodes.is_empty() {
        return Err("Federal Reserve calendar year panels were not found".to_owned());
    }

    let mut entries = Vec::new();
    for panel in panel_nodes {
        let heading = panel
            .select(&headings)
            .next()
            .map(|element| element_text(&element))
            .ok_or_else(|| "Federal Reserve calendar year heading was missing".to_owned())?;
        let year = crate::parser::parse_year(&heading).map_err(|error| error.to_string())?;
        for meeting in panel.select(&meetings) {
            let month = meeting
                .select(&months)
                .next()
                .map(|element| element_text(&element))
                .ok_or_else(|| "Federal Reserve calendar month was missing".to_owned())?;
            let date_text = meeting
                .select(&dates)
                .next()
                .map(|element| element_text(&element))
                .ok_or_else(|| "Federal Reserve calendar date was missing".to_owned())?;
            if date_text.to_ascii_lowercase().contains("notation vote") {
                continue;
            }
            let publication_date = parse_statement_date(year, &month, &date_text)
                .map_err(|error| error.to_string())?;
            let Some(statement_column) = meeting.select(&statement_columns).next() else {
                continue;
            };
            let Some(title) = statement_column
                .select(&labels)
                .map(|label| element_text(&label))
                .find(|label| {
                    label
                        .trim_end_matches(':')
                        .eq_ignore_ascii_case("statement")
                })
                .map(|label| label.trim_end_matches(':').to_owned())
            else {
                continue;
            };
            let Some(link) = statement_column.select(&links).find(|link| {
                element_text(link).eq_ignore_ascii_case("html")
                    && link
                        .value()
                        .attr("href")
                        .is_some_and(|href| href.contains("/pressreleases/"))
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
            entries.push(StatementEntry {
                publication_date,
                title,
                url,
                archive_url: page_url.to_owned(),
            });
        }
    }
    Ok(entries)
}

pub(super) fn scheduled_meeting_dates(html: &str) -> Result<HashSet<NaiveDate>, String> {
    let document = Html::parse_document(html);
    let panels = selector("#article .panel.panel-default")?;
    let headings = selector(".panel-heading h4")?;
    let meetings = selector(".fomc-meeting")?;
    let months = selector(".fomc-meeting__month")?;
    let dates = selector(".fomc-meeting__date")?;
    let statement_columns = selector(".col-lg-2")?;
    let links = selector("a[href]")?;
    let panel_nodes = document.select(&panels).collect::<Vec<_>>();
    if panel_nodes.is_empty() {
        return Err("Federal Reserve calendar year panels were not found".to_owned());
    }

    let mut scheduled_dates = HashSet::new();
    for panel in panel_nodes {
        let heading = panel
            .select(&headings)
            .next()
            .map(|element| element_text(&element))
            .ok_or_else(|| "Federal Reserve calendar year heading was missing".to_owned())?;
        let year = crate::parser::parse_year(&heading).map_err(|error| error.to_string())?;
        for meeting in panel.select(&meetings) {
            let month = meeting
                .select(&months)
                .next()
                .map(|element| element_text(&element))
                .ok_or_else(|| "Federal Reserve calendar month was missing".to_owned())?;
            let date_text = meeting
                .select(&dates)
                .next()
                .map(|element| element_text(&element))
                .ok_or_else(|| "Federal Reserve calendar date was missing".to_owned())?;
            if date_text.to_ascii_lowercase().contains("notation vote") {
                continue;
            }
            let publication_date = crate::parser::parse_statement_date(year, &month, &date_text)
                .map_err(|error| error.to_string())?;
            let has_statement = meeting
                .select(&statement_columns)
                .next()
                .is_some_and(|column| {
                    column.select(&links).any(|link| {
                        element_text(&link).eq_ignore_ascii_case("html")
                            && link
                                .value()
                                .attr("href")
                                .is_some_and(|href| href.contains("/pressreleases/"))
                    })
                });
            if !has_statement {
                scheduled_dates.insert(publication_date);
            }
        }
    }
    Ok(scheduled_dates)
}

pub(super) fn historical_year_pages(
    html: &str,
    page_url: &str,
) -> Result<HashMap<i32, String>, String> {
    let document = Html::parse_document(html);
    let links = selector("a[href]")?;
    let mut pages = HashMap::new();
    for link in document.select(&links) {
        let Ok(year) = element_text(&link).parse::<i32>() else {
            continue;
        };
        let Some(url) = link
            .value()
            .attr("href")
            .and_then(|href| OfficialHttpClient::resolve_url(page_url, href))
        else {
            continue;
        };
        pages.insert(year, url);
    }
    if pages.is_empty() {
        return Err("Federal Reserve historical year links were not found".to_owned());
    }
    Ok(pages)
}

pub(super) fn historical_statement_entries(
    html: &str,
    page_url: &str,
    year: i32,
) -> Result<Vec<StatementEntry>, String> {
    let document = Html::parse_document(html);
    let meeting_headings = selector("h5")?;
    let links = selector("a[href]")?;
    let mut entries = Vec::new();
    for heading in document.select(&meeting_headings) {
        let heading_text = element_text(&heading);
        let Some(publication_date) = parse_historical_meeting_date(&heading_text, year) else {
            continue;
        };
        let mut siblings = heading.next_siblings();
        for sibling in siblings.by_ref() {
            if sibling
                .value()
                .as_element()
                .is_some_and(|element| element.name() == "h5")
            {
                break;
            }
            let Some(element) = ElementRef::wrap(sibling) else {
                continue;
            };
            for link in element
                .select(&links)
                .filter(|link| element_text(link).eq_ignore_ascii_case("statement"))
            {
                let Some(url) = link
                    .value()
                    .attr("href")
                    .and_then(|href| OfficialHttpClient::resolve_url(page_url, href))
                else {
                    continue;
                };
                entries.push(StatementEntry {
                    publication_date,
                    title: element_text(&link),
                    url,
                    archive_url: page_url.to_owned(),
                });
            }
        }
    }
    if document.select(&meeting_headings).next().is_none() {
        return Err("Federal Reserve historical meeting headings were not found".to_owned());
    }
    Ok(entries)
}

fn parse_historical_meeting_date(value: &str, year: i32) -> Option<NaiveDate> {
    if !value.to_ascii_lowercase().contains("meeting")
        || value.to_ascii_lowercase().contains("cancelled")
        || value.to_ascii_lowercase().contains("notation vote")
    {
        return None;
    }
    let heading = value.split_once(" Meeting")?.0;
    let mut parts = heading.split_whitespace();
    let month = parts.next()?;
    let date_text = parts.collect::<Vec<_>>().join(" ");
    let Some((start_day_text, end_text)) = date_text.split_once('-') else {
        return parse_statement_date(year, month, &date_text).ok();
    };
    let start_month = crate::parser::parse_month(month).ok()?;
    let start_day = start_day_text.trim().parse::<u32>().ok()?;
    let mut end_parts = end_text.split_whitespace();
    let first_end_part = end_parts.next()?;
    let explicit_end_month = crate::parser::parse_month(first_end_part).ok();
    let Some(end_month) = explicit_end_month else {
        return parse_statement_date(year, month, &date_text).ok();
    };
    let end_day = end_parts.next()?.trim().parse::<u32>().ok()?;
    let start_date = NaiveDate::from_ymd_opt(year, start_month, start_day)?;
    let end_year = if end_month < start_month {
        year.checked_add(1)?
    } else {
        year
    };
    let end_date = NaiveDate::from_ymd_opt(end_year, end_month, end_day)?;
    (end_date >= start_date).then_some(end_date)
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
    use super::{
        StatementEntry, calendar_statement_entries, historical_statement_entries,
        historical_year_pages, scheduled_meeting_dates,
    };
    use chrono::NaiveDate;

    const ORIGIN: &str = "https://www.federalreserve.gov";
    const CALENDAR_URL: &str = "https://www.federalreserve.gov/monetarypolicy/fomccalendars.htm";
    const YEAR_INDEX_URL: &str =
        "https://www.federalreserve.gov/monetarypolicy/fomc_historical_year.htm";
    const CALENDAR_FIXTURE: &str = include_str!("../fixtures/calendar_reference_synthetic.html");
    const YEAR_INDEX_FIXTURE: &str =
        include_str!("../fixtures/historical_year_index_synthetic.html");
    const ARCHIVE_FIXTURE: &str = include_str!("../fixtures/historical_archive_synthetic.html");

    #[test]
    fn current_calendar_links_statement_html_to_the_meeting_end_date() {
        assert_eq!(
            calendar_statement_entries(CALENDAR_FIXTURE, CALENDAR_URL).expect("parse calendar"),
            vec![StatementEntry {
                publication_date: NaiveDate::from_ymd_opt(2099, 7, 9).unwrap_or(NaiveDate::MIN),
                title: "Statement".to_owned(),
                url: format!("{ORIGIN}/newsevents/pressreleases/fictional-statement.html"),
                archive_url: CALENDAR_URL.to_owned(),
            }],
        );
    }

    #[test]
    fn historical_archive_links_are_selected_from_the_official_year_index() {
        assert_eq!(
            historical_year_pages(YEAR_INDEX_FIXTURE, YEAR_INDEX_URL).expect("year links"),
            [(
                2099,
                format!("{ORIGIN}/monetarypolicy/fictional-period.html")
            )]
            .into_iter()
            .collect(),
        );
    }

    #[test]
    fn historical_archive_matches_fictional_meeting_statement() {
        assert_eq!(
            historical_statement_entries(
                ARCHIVE_FIXTURE,
                "https://www.federalreserve.gov/monetarypolicy/fictional-period.html",
                2099,
            )
            .expect("historical entries"),
            vec![
                StatementEntry {
                    publication_date: NaiveDate::from_ymd_opt(2099, 4, 8).unwrap_or(NaiveDate::MIN),
                    title: "Statement".to_owned(),
                    url: format!("{ORIGIN}/newsevents/pressreleases/fictional-statement.html"),
                    archive_url:
                        "https://www.federalreserve.gov/monetarypolicy/fictional-period.html"
                            .to_owned(),
                },
                StatementEntry {
                    publication_date: NaiveDate::from_ymd_opt(2099, 7, 14)
                        .unwrap_or(NaiveDate::MIN),
                    title: "Statement".to_owned(),
                    url: format!("{ORIGIN}/newsevents/pressreleases/fictional-next-statement.html"),
                    archive_url:
                        "https://www.federalreserve.gov/monetarypolicy/fictional-period.html"
                            .to_owned(),
                },
                StatementEntry {
                    publication_date: NaiveDate::from_ymd_opt(2099, 3, 1).unwrap_or(NaiveDate::MIN),
                    title: "Statement".to_owned(),
                    url: format!("{ORIGIN}/newsevents/pressreleases/fictional-cross-month.html"),
                    archive_url:
                        "https://www.federalreserve.gov/monetarypolicy/fictional-period.html"
                            .to_owned(),
                },
            ],
        );
    }

    #[test]
    fn calendar_rows_without_statement_links_remain_official_schedule_dates() {
        assert_eq!(
            scheduled_meeting_dates(CALENDAR_FIXTURE).expect("parse calendar schedule"),
            [NaiveDate::from_ymd_opt(2099, 9, 18).unwrap_or(NaiveDate::MIN)]
                .into_iter()
                .collect(),
        );
    }
}
