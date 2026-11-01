mod official_http;
mod parser;

use crate::official_http::OfficialHttpClient;
use async_trait::async_trait;
use chrono::{Datelike, NaiveDate};
use core_application::{
    calendar::macro_reference_targets::MacroReferenceTarget,
    macro_reference_source::{
        MacroOfficialReference, MacroReferenceFetchResult, MacroReferenceSource,
        MacroReferenceSourceError,
    },
};
use core_domain::calendar_event::CalendarEventReferenceKind;
use std::collections::{HashMap, HashSet};

const HISTORIC_RELEASES_URL: &str = "https://www.census.gov/retail/marts/historic_releases.html";
const RELEASE_DATES_URL: &str = "https://www.census.gov/retail/marts/www/MARTSreleasedates.xlsx";
const PERIOD_SEARCH_WINDOW_MONTHS: i64 = 6;
const MAX_PDF_FETCHES_PER_BATCH: usize = 180;

#[derive(Clone)]
pub struct CensusMacroReferenceSource {
    http: OfficialHttpClient,
}

impl CensusMacroReferenceSource {
    pub fn new(redis_url: &str) -> Result<Self, String> {
        OfficialHttpClient::new(redis_url, "census.gov")
            .map(|http| Self { http })
            .map_err(|error| error.to_string())
    }
}

#[async_trait]
impl MacroReferenceSource for CensusMacroReferenceSource {
    fn supports(&self, target: &MacroReferenceTarget) -> bool {
        supports_target(target)
    }

    async fn fetch_references(
        &self,
        targets: &[MacroReferenceTarget],
    ) -> Result<Vec<MacroReferenceFetchResult>, MacroReferenceSourceError> {
        let supported_targets = targets
            .iter()
            .filter(|target| self.supports(target))
            .cloned()
            .collect::<Vec<_>>();
        if supported_targets.is_empty() {
            return Ok(Vec::new());
        }

        let archive_html = match self.http.get_text_cached(HISTORIC_RELEASES_URL).await {
            Ok(html) => html,
            Err(error) => {
                let failure = error.to_string();
                return Ok(supported_targets
                    .into_iter()
                    .map(|target| MacroReferenceFetchResult::retry(target, failure.clone()))
                    .collect());
            }
        };
        let Some(archive_links) = parser::parse_archive_links(&archive_html, HISTORIC_RELEASES_URL)
        else {
            return Ok(supported_targets
                .into_iter()
                .map(|target| {
                    MacroReferenceFetchResult::retry(
                        target,
                        "Census historic release archive response could not be parsed",
                    )
                })
                .collect());
        };
        let mut seen_pdf_urls = HashSet::new();
        let pdf_links = archive_links
            .into_iter()
            .filter(|link| {
                link.url
                    .to_ascii_lowercase()
                    .split(['?', '#'])
                    .next()
                    .is_some_and(|path| path.ends_with(".pdf"))
            })
            .filter(|link| link.period_start.is_some())
            .filter(|link| seen_pdf_urls.insert(link.url.clone()))
            .collect::<Vec<_>>();

        let release_dates_bytes = match self.http.get_bytes_cached(RELEASE_DATES_URL).await {
            Ok(bytes) => bytes,
            Err(error) => {
                let failure = error.to_string();
                return Ok(supported_targets
                    .into_iter()
                    .map(|target| MacroReferenceFetchResult::retry(target, failure.clone()))
                    .collect());
            }
        };
        let Some(release_dates) = parser::parse_release_date_workbook(&release_dates_bytes) else {
            return Ok(supported_targets
                .into_iter()
                .map(|target| {
                    MacroReferenceFetchResult::retry(
                        target,
                        "Census historical release dates workbook could not be parsed",
                    )
                })
                .collect());
        };

        let mut pdf_cache = HashMap::<String, Option<parser::CensusPdfPublication>>::new();
        let mut fetched_pdf_count = 0;
        let mut results = Vec::with_capacity(supported_targets.len());
        for target in supported_targets {
            let Some(release_date) = parse_fred_external_id(&target.event_external_id) else {
                continue;
            };
            if release_dates.binary_search(&release_date).is_err() {
                results.push(listing_result(&target));
                continue;
            }
            let candidate_links = pdf_links
                .iter()
                .filter(|link| {
                    link.period_start.is_some_and(|period_start| {
                        is_near_release_month(period_start, release_date)
                    })
                })
                .collect::<Vec<_>>();
            if candidate_links.is_empty() {
                results.push(listing_result(&target));
                continue;
            }

            let mut documents = Vec::new();
            let mut ambiguous_candidate = false;
            let mut budget_exhausted = false;
            let mut lookup_failure = None;
            for link in candidate_links {
                let publication = if let Some(cached) = pdf_cache.get(&link.url) {
                    cached.clone()
                } else if fetched_pdf_count >= MAX_PDF_FETCHES_PER_BATCH {
                    budget_exhausted = true;
                    break;
                } else {
                    fetched_pdf_count += 1;
                    match self.http.get_bytes_cached(&link.url).await {
                        Ok(bytes) => {
                            let publication = parser::parse_pdf_publication(&bytes);
                            pdf_cache.insert(link.url.clone(), publication.clone());
                            publication
                        }
                        Err(error) => {
                            lookup_failure = Some(error.to_string());
                            break;
                        }
                    }
                };
                let Some(publication) = publication else {
                    ambiguous_candidate = true;
                    continue;
                };
                if publication.release_date == release_date {
                    if let Some(document) = parser::census_document(&target, link, &publication) {
                        documents.push(document);
                    } else {
                        ambiguous_candidate = true;
                    }
                }
            }
            if let Some(failure) = lookup_failure {
                results.push(MacroReferenceFetchResult::retry(target, failure));
            } else if budget_exhausted {
                results.push(MacroReferenceFetchResult::retry(
                    target,
                    "Census PDF lookup budget exhausted",
                ));
            } else if !documents.is_empty() && !ambiguous_candidate {
                results.push(MacroReferenceFetchResult::success(target, documents));
            } else {
                results.push(listing_result(&target));
            }
        }
        Ok(results)
    }
}

fn is_near_release_month(period_start: NaiveDate, release_date: NaiveDate) -> bool {
    let period_month = i64::from(period_start.year()) * 12 + i64::from(period_start.month());
    let release_month = i64::from(release_date.year()) * 12 + i64::from(release_date.month());
    (period_month - release_month).abs() <= PERIOD_SEARCH_WINDOW_MONTHS
}

fn parse_fred_external_id(external_id: &str) -> Option<NaiveDate> {
    let (release_id, release_date) = external_id.split_once(':')?;
    if release_id != "9" || release_date.len() != 10 {
        return None;
    }
    NaiveDate::parse_from_str(release_date, "%Y-%m-%d").ok()
}

fn supports_target(target: &MacroReferenceTarget) -> bool {
    target.event_source == "fred" && parse_fred_external_id(&target.event_external_id).is_some()
}

fn listing_result(target: &MacroReferenceTarget) -> MacroReferenceFetchResult {
    MacroReferenceFetchResult::success(
        target.clone(),
        vec![MacroOfficialReference {
            target: target.clone(),
            kind: CalendarEventReferenceKind::OfficialListing,
            source: "census".to_owned(),
            external_id: format!("retail-release-listing:{}", target.event_external_id),
            title: "Official retail release archive".to_owned(),
            url: Some(HISTORIC_RELEASES_URL.to_owned()),
            target_period_start: None,
            target_period_end: None,
            target_period_type: None,
            publication_evidence: None,
            publication_evidence_url: None,
        }],
    )
}

#[cfg(test)]
mod tests {
    use core_application::{
        calendar::macro_reference_targets::MacroReferenceTarget,
        macro_reference_source::MacroOfficialReference,
    };
    use core_domain::calendar_event::CalendarEventReferenceKind;
    use indoc::indoc;
    use rstest::rstest;

    use super::{HISTORIC_RELEASES_URL, listing_result, parse_fred_external_id, supports_target};
    use crate::parser::parse_archive_links;

    fn target(external_id: &str) -> MacroReferenceTarget {
        MacroReferenceTarget {
            event_source: "fred".to_owned(),
            event_external_id: external_id.to_owned(),
            event_title: "Synthetic release event".to_owned(),
            same_day_event_count: 1,
        }
    }

    #[rstest]
    #[case::retail("9:2099-04-08", Some("2099-04-08"))]
    #[case::other_release("10:2099-04-08", None)]
    #[case::invalid_date("9:2099-02-31", None)]
    fn only_census_retail_event_ids_are_supported(
        #[case] external_id: &str,
        #[case] expected: Option<&str>,
    ) {
        let actual =
            parse_fred_external_id(external_id).map(|date| date.format("%Y-%m-%d").to_string());
        assert_eq!(actual, expected.map(str::to_owned));
    }

    #[test]
    fn successful_ambiguous_lookup_returns_the_official_archive() {
        let target = target("9:2099-04-08");
        let actual = listing_result(&target);

        assert_eq!(
            actual,
            core_application::macro_reference_source::MacroReferenceFetchResult::success(
                target.clone(),
                vec![MacroOfficialReference {
                    target,
                    kind: CalendarEventReferenceKind::OfficialListing,
                    source: "census".to_owned(),
                    external_id: "retail-release-listing:9:2099-04-08".to_owned(),
                    title: "Official retail release archive".to_owned(),
                    url: Some(HISTORIC_RELEASES_URL.to_owned()),
                    target_period_start: None,
                    target_period_end: None,
                    target_period_type: None,
                    publication_evidence: None,
                    publication_evidence_url: None,
                }],
            ),
        );
    }

    #[test]
    fn archive_parser_keeps_pdf_links_and_leaves_date_proof_to_the_documents() {
        let archive = indoc! {
            r#"
            <html><body><a href="/retail/sample-period.pdf">Synthetic period report</a>
            <a href="/retail/sample-dates.xlsx">Synthetic release dates</a></body></html>
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
                    "Synthetic period report".to_owned(),
                    None,
                    "https://example.invalid/retail/sample-period.pdf".to_owned(),
                ),
                (
                    "Synthetic release dates".to_owned(),
                    None,
                    "https://example.invalid/retail/sample-dates.xlsx".to_owned(),
                ),
            ]),
        );
    }

    #[test]
    fn census_source_only_supports_the_retail_release_id() {
        let actual = [
            ("9:2099-04-08", true),
            ("53:2099-04-08", false),
            ("9:2099-02-31", false),
        ]
        .map(|(external_id, expected)| {
            let target = target(external_id);
            (supports_target(&target), expected)
        });

        assert_eq!(actual, [(true, true), (false, false), (false, false)]);
    }
}
