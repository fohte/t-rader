mod official_http;
mod parser;

use std::collections::HashMap;

use crate::official_http::OfficialHttpClient;
use async_trait::async_trait;
use chrono::NaiveDate;
use core_application::{
    calendar::macro_reference_targets::MacroReferenceTarget,
    macro_reference_source::{
        MacroOfficialReference, MacroReferenceFetchResult, MacroReferenceSource,
        MacroReferenceSourceError,
    },
};
use core_domain::calendar_event::CalendarEventReferenceKind;

use crate::parser::{
    bls_document, parse_bls_archive_rows, parse_bls_publication, parse_fred_external_id,
};

const CPI_ARCHIVE_URL: &str = "https://www.bls.gov/bls/news-release/cpi.htm";
const EMPLOYMENT_ARCHIVE_URL: &str = "https://www.bls.gov/bls/news-release/empsit.htm";

#[derive(Clone)]
pub struct BlsMacroReferenceSource {
    http: OfficialHttpClient,
}

impl BlsMacroReferenceSource {
    pub fn new(redis_url: &str) -> Result<Self, String> {
        OfficialHttpClient::new(redis_url)
            .map(|http| Self { http })
            .map_err(|error| error.to_string())
    }

    async fn fetch_release_group(
        &self,
        release_id: &str,
        release_targets: Vec<(MacroReferenceTarget, NaiveDate)>,
    ) -> Vec<MacroReferenceFetchResult> {
        let archive_url = archive_url(release_id);
        let archive_page = match self.http.get_text_cached(archive_url).await {
            Ok(page) => page,
            Err(error) => {
                return release_targets
                    .into_iter()
                    .map(|(target, _)| MacroReferenceFetchResult::retry(target, error.to_string()))
                    .collect();
            }
        };
        let Some(rows) = parse_bls_archive_rows(&archive_page, archive_url, release_id) else {
            return release_targets
                .into_iter()
                .map(|(target, _)| {
                    MacroReferenceFetchResult::retry(
                        target,
                        "BLS archive response could not be parsed",
                    )
                })
                .collect();
        };

        let mut results = Vec::new();
        for (target, release_date) in release_targets {
            let matching_rows = rows
                .iter()
                .filter(|row| row.release_date == release_date)
                .collect::<Vec<_>>();
            let mut documents = Vec::new();
            let mut failed_lookup = None;
            for row in &matching_rows {
                let page = match self.http.get_text_cached(&row.url).await {
                    Ok(page) => page,
                    Err(error) => {
                        failed_lookup = Some(error.to_string());
                        break;
                    }
                };
                let Some(publication) = parse_bls_publication(&page, release_id) else {
                    continue;
                };
                if let Some(document) =
                    bls_document(&target, release_id, release_date, row, &publication)
                {
                    documents.push(document);
                }
            }
            if let Some(error) = failed_lookup {
                results.push(MacroReferenceFetchResult::retry(target, error));
                continue;
            }
            results.push(result_for_matches(
                &target,
                release_id,
                archive_url,
                matching_rows.len(),
                documents,
            ));
        }
        results
    }
}

#[async_trait]
impl MacroReferenceSource for BlsMacroReferenceSource {
    fn supports(&self, target: &MacroReferenceTarget) -> bool {
        supports_target(target)
    }

    async fn fetch_references(
        &self,
        targets: &[MacroReferenceTarget],
    ) -> Result<Vec<MacroReferenceFetchResult>, MacroReferenceSourceError> {
        let mut grouped = HashMap::<String, Vec<(MacroReferenceTarget, NaiveDate)>>::new();
        for target in targets {
            let Some((release_id, release_date)) =
                parse_fred_external_id(&target.event_external_id)
            else {
                continue;
            };
            if matches!(release_id, "10" | "50") {
                grouped
                    .entry(release_id.to_owned())
                    .or_default()
                    .push((target.clone(), release_date));
            }
        }

        let mut results = Vec::new();
        for (release_id, release_targets) in grouped {
            results.extend(self.fetch_release_group(&release_id, release_targets).await);
        }
        Ok(results)
    }
}

fn archive_url(release_id: &str) -> &'static str {
    match release_id {
        "10" => CPI_ARCHIVE_URL,
        _ => EMPLOYMENT_ARCHIVE_URL,
    }
}

fn supports_target(target: &MacroReferenceTarget) -> bool {
    target.event_source == "fred"
        && parse_fred_external_id(&target.event_external_id)
            .is_some_and(|(release_id, _)| matches!(release_id, "10" | "50"))
}

fn result_for_matches(
    target: &MacroReferenceTarget,
    release_id: &str,
    archive_url: &str,
    matching_row_count: usize,
    documents: Vec<MacroOfficialReference>,
) -> MacroReferenceFetchResult {
    if !documents.is_empty() && documents.len() == matching_row_count {
        MacroReferenceFetchResult::success(target.clone(), documents)
    } else {
        MacroReferenceFetchResult::success(
            target.clone(),
            vec![listing_reference(target, release_id, archive_url)],
        )
    }
}

fn listing_reference(
    target: &MacroReferenceTarget,
    release_id: &str,
    url: &str,
) -> MacroOfficialReference {
    MacroOfficialReference {
        target: target.clone(),
        kind: CalendarEventReferenceKind::OfficialListing,
        source: "bls".to_owned(),
        external_id: format!(
            "bls-release-listing:{release_id}:{}",
            target.event_external_id
        ),
        title: "Official release archive".to_owned(),
        url: Some(url.to_owned()),
        target_period_start: None,
        target_period_end: None,
        target_period_type: None,
        publication_evidence: None,
        publication_evidence_url: None,
    }
}

#[cfg(test)]
mod tests {
    use core_application::{
        calendar::macro_reference_targets::MacroReferenceTarget,
        macro_reference_source::{MacroOfficialReference, MacroReferenceFetchResult},
    };
    use core_domain::calendar_event::CalendarEventReferenceKind;
    use rstest::rstest;

    use super::{CPI_ARCHIVE_URL, result_for_matches, supports_target};

    fn target(event_source: &str, external_id: &str) -> MacroReferenceTarget {
        MacroReferenceTarget {
            event_source: event_source.to_owned(),
            event_external_id: external_id.to_owned(),
            event_title: "Synthetic release event".to_owned(),
            same_day_event_count: 1,
        }
    }

    #[rstest]
    #[case::consumer_price("fred", "10:2099-04-08", true)]
    #[case::employment("fred", "50:2099-04-08", true)]
    #[case::other_release("fred", "53:2099-04-08", false)]
    #[case::other_source("sample", "10:2099-04-08", false)]
    fn source_supports_only_its_fred_releases(
        #[case] event_source: &str,
        #[case] external_id: &str,
        #[case] expected: bool,
    ) {
        assert_eq!(
            supports_target(&target(event_source, external_id)),
            expected
        );
    }

    #[test]
    fn ambiguous_matching_rows_use_the_official_archive() {
        let target = target("fred", "10:2099-04-08");
        let actual = result_for_matches(&target, "10", CPI_ARCHIVE_URL, 1, Vec::new());

        assert_eq!(
            actual,
            MacroReferenceFetchResult::success(
                target.clone(),
                vec![MacroOfficialReference {
                    target,
                    kind: CalendarEventReferenceKind::OfficialListing,
                    source: "bls".to_owned(),
                    external_id: "bls-release-listing:10:10:2099-04-08".to_owned(),
                    title: "Official release archive".to_owned(),
                    url: Some(CPI_ARCHIVE_URL.to_owned()),
                    target_period_start: None,
                    target_period_end: None,
                    target_period_type: None,
                    publication_evidence: None,
                    publication_evidence_url: None,
                }],
            ),
        );
    }
}
