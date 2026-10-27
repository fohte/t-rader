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

use crate::parser::{bea_document, bea_listing_url, parse_bea_rows, parse_fred_external_id};

#[derive(Clone)]
pub struct BeaMacroReferenceSource {
    http: OfficialHttpClient,
}

impl BeaMacroReferenceSource {
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
        let Some(archive_url) = bea_listing_url(release_id) else {
            return release_targets
                .into_iter()
                .map(|(target, _)| {
                    MacroReferenceFetchResult::retry(target, "unsupported BEA release identifier")
                })
                .collect();
        };
        let archive_page = match self.http.get_text_cached(&archive_url).await {
            Ok(page) => page,
            Err(error) => {
                return release_targets
                    .into_iter()
                    .map(|(target, _)| MacroReferenceFetchResult::retry(target, error.to_string()))
                    .collect();
            }
        };
        let Some(rows) = parse_bea_rows(&archive_page, &archive_url) else {
            return release_targets
                .into_iter()
                .map(|(target, _)| {
                    MacroReferenceFetchResult::retry(
                        target,
                        "BEA archive response could not be parsed",
                    )
                })
                .collect();
        };

        release_targets
            .into_iter()
            .map(|(target, release_date)| {
                let matching_rows = rows
                    .iter()
                    .filter(|row| row.published_at.date_naive() == release_date)
                    .collect::<Vec<_>>();
                let documents = matching_rows
                    .iter()
                    .filter_map(|row| {
                        bea_document(&target, release_id, release_date, row, &archive_url)
                    })
                    .collect::<Vec<_>>();
                result_for_matches(
                    &target,
                    release_id,
                    &archive_url,
                    matching_rows.len(),
                    documents,
                )
            })
            .collect()
    }
}

#[async_trait]
impl MacroReferenceSource for BeaMacroReferenceSource {
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
            if matches!(release_id, "53" | "54") {
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

fn supports_target(target: &MacroReferenceTarget) -> bool {
    target.event_source == "fred"
        && parse_fred_external_id(&target.event_external_id)
            .is_some_and(|(release_id, _)| matches!(release_id, "53" | "54"))
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
        source: "bea".to_owned(),
        external_id: format!(
            "bea-release-listing:{release_id}:{}",
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

    use super::{result_for_matches, supports_target};

    fn target(event_source: &str, external_id: &str) -> MacroReferenceTarget {
        MacroReferenceTarget {
            event_source: event_source.to_owned(),
            event_external_id: external_id.to_owned(),
            event_title: "Synthetic release event".to_owned(),
            same_day_event_count: 1,
        }
    }

    #[rstest]
    #[case::gross_domestic_product("fred", "53:2099-04-08", true)]
    #[case::personal_consumption("fred", "54:2099-04-08", true)]
    #[case::other_release("fred", "10:2099-04-08", false)]
    #[case::other_source("sample", "53:2099-04-08", false)]
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

    #[rstest]
    #[case::past_scheduled_without_publication("53:2001-04-08", 0)]
    #[case::publication_record_without_document("53:2099-04-08", 1)]
    fn missing_individual_publication_uses_the_official_archive(
        #[case] external_id: &str,
        #[case] matching_row_count: usize,
    ) {
        let target = target("fred", external_id);
        let archive_url = "https://example.invalid/news/archive";
        let actual = result_for_matches(&target, "53", archive_url, matching_row_count, Vec::new());

        assert_eq!(
            actual,
            MacroReferenceFetchResult::success(
                target.clone(),
                vec![MacroOfficialReference {
                    target,
                    kind: CalendarEventReferenceKind::OfficialListing,
                    source: "bea".to_owned(),
                    external_id: format!("bea-release-listing:53:{external_id}"),
                    title: "Official release archive".to_owned(),
                    url: Some(archive_url.to_owned()),
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
