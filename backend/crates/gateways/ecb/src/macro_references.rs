mod parser;

use std::collections::{HashMap, HashSet};

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

use crate::official_http::OfficialHttpClient;

const SOURCE: &str = "ecb";
const DECISION_ARCHIVE_URL: &str =
    "https://www.ecb.europa.eu/press/govcdec/mopo/html/index.en.html";
const PREVIOUS_DECISION_ARCHIVE_URL: &str =
    "https://www.ecb.europa.eu/press/govcdec/mopo/previous/html/index.en.html";
const MEETING_SCHEDULE_URL: &str =
    "https://www.ecb.europa.eu/press/calendars/mgcgc/html/index.en.html";

pub struct EcbMacroReferenceSource {
    http: OfficialHttpClient,
}

impl EcbMacroReferenceSource {
    pub fn new(redis_url: &str) -> Result<Self, String> {
        Ok(Self {
            http: OfficialHttpClient::new(redis_url)?,
        })
    }
}

#[async_trait]
impl MacroReferenceSource for EcbMacroReferenceSource {
    fn supports(&self, target: &MacroReferenceTarget) -> bool {
        target.event_source == SOURCE && target_date(target).is_some()
    }

    async fn fetch_references(
        &self,
        targets: &[MacroReferenceTarget],
    ) -> Result<Vec<MacroReferenceFetchResult>, MacroReferenceSourceError> {
        let supported = targets
            .iter()
            .filter(|target| self.supports(target))
            .cloned()
            .collect::<Vec<_>>();
        if supported.is_empty() {
            return Ok(Vec::new());
        }

        let calendar_html = self
            .http
            .get_text(MEETING_SCHEDULE_URL)
            .await
            .map_err(source_error)?;
        let (calendar_events, _) = crate::parser::parse_calendar_events(&calendar_html)
            .map_err(|error| source_error(error.to_string()))?;

        let current_index_html = self
            .http
            .get_text(DECISION_ARCHIVE_URL)
            .await
            .map_err(source_error)?;
        let current_pages =
            parser::decision_archive_pages(&current_index_html, DECISION_ARCHIVE_URL)
                .map_err(source_error)?;
        let years = supported
            .iter()
            .filter_map(|target| target_date(target).map(|date| date.year()))
            .collect::<HashSet<_>>();
        let needs_previous = years.iter().any(|year| !current_pages.contains_key(year));
        let previous_pages = if needs_previous {
            let previous_index_html = self
                .http
                .get_text(PREVIOUS_DECISION_ARCHIVE_URL)
                .await
                .map_err(source_error)?;
            parser::decision_archive_pages(&previous_index_html, PREVIOUS_DECISION_ARCHIVE_URL)
                .map_err(source_error)?
        } else {
            HashMap::new()
        };
        let mut candidates = HashMap::new();
        for year in years {
            let Some(page_url) = current_pages
                .get(&year)
                .or_else(|| previous_pages.get(&year))
            else {
                continue;
            };
            let html = self.http.get_text(page_url).await.map_err(source_error)?;
            let entries = parser::decision_entries(&html, page_url).map_err(source_error)?;
            for target in supported
                .iter()
                .filter(|target| target_date(target).is_some_and(|date| date.year() == year))
            {
                if let Some(date) = target_date(target) {
                    candidates.insert(
                        target.event_external_id.clone(),
                        entries
                            .iter()
                            .filter(|entry| entry.publication_date == date)
                            .cloned()
                            .collect::<Vec<_>>(),
                    );
                }
            }
        }

        Ok(supported
            .into_iter()
            .map(|target| {
                let matched = if is_decision_day(&target.event_title) {
                    candidates
                        .get(&target.event_external_id)
                        .into_iter()
                        .flatten()
                        .collect::<Vec<_>>()
                } else {
                    Vec::new()
                };
                let references = if matched.len() == 1 {
                    matched
                        .first()
                        .map(|entry| vec![decision_reference(&target, entry)])
                        .unwrap_or_default()
                } else {
                    let scheduled = calendar_events
                        .iter()
                        .any(|event| event.external_id == target.event_external_id);
                    vec![listing_reference(&target, scheduled)]
                };
                MacroReferenceFetchResult::success(target, references)
            })
            .collect())
    }
}

fn decision_reference(
    target: &MacroReferenceTarget,
    entry: &parser::DecisionEntry,
) -> MacroOfficialReference {
    MacroOfficialReference {
        target: target.clone(),
        kind: CalendarEventReferenceKind::IndividualDocument,
        source: SOURCE.to_owned(),
        external_id: entry.url.clone(),
        title: entry.title.clone(),
        url: Some(entry.url.clone()),
        target_period_start: Some(entry.publication_date),
        target_period_end: Some(entry.publication_date),
        target_period_type: Some("meeting".to_owned()),
        publication_evidence: Some(format!(
            "公式アーカイブに公表日 {} と資料名「{}」を確認",
            entry.publication_date, entry.title
        )),
        publication_evidence_url: Some(entry.archive_url.clone()),
    }
}

fn listing_reference(target: &MacroReferenceTarget, scheduled: bool) -> MacroOfficialReference {
    let (url, title) = if scheduled {
        (MEETING_SCHEDULE_URL, "ECB の公式会合日程")
    } else {
        (DECISION_ARCHIVE_URL, "ECB 金融政策決定の公式一覧")
    };
    MacroOfficialReference {
        target: target.clone(),
        kind: CalendarEventReferenceKind::OfficialListing,
        source: SOURCE.to_owned(),
        external_id: format!("{}:official-listing", target.event_external_id),
        title: title.to_owned(),
        url: Some(url.to_owned()),
        target_period_start: None,
        target_period_end: None,
        target_period_type: Some(
            if scheduled {
                "official_schedule"
            } else {
                "official_archive"
            }
            .to_owned(),
        ),
        publication_evidence: None,
        publication_evidence_url: None,
    }
}

fn target_date(target: &MacroReferenceTarget) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(&target.event_external_id, "%Y-%m-%d").ok()
}

fn is_decision_day(title: &str) -> bool {
    title
        .to_ascii_lowercase()
        .contains("followed by press conference")
}

fn source_error(error: String) -> MacroReferenceSourceError {
    MacroReferenceSourceError::new(error)
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;
    use core_application::calendar::macro_reference_targets::MacroReferenceTarget;
    use core_domain::calendar_event::CalendarEventReferenceKind;

    use super::{is_decision_day, listing_reference, target_date};

    fn target(event_source: &str, external_id: &str, title: &str) -> MacroReferenceTarget {
        MacroReferenceTarget {
            event_source: event_source.to_owned(),
            event_external_id: external_id.to_owned(),
            event_title: title.to_owned(),
            same_day_event_count: 1,
        }
    }

    #[test]
    fn meeting_first_day_uses_schedule_without_a_decision_document() {
        let reference = listing_reference(
            &target(
                "ecb",
                "2099-04-08",
                "Governing Council of the ECB: monetary policy meeting (Day 1)",
            ),
            true,
        );

        assert_eq!(
            (
                reference.kind,
                reference.url.as_deref(),
                reference.target_period_type.as_deref(),
                reference.publication_evidence.as_deref(),
            ),
            (
                CalendarEventReferenceKind::OfficialListing,
                Some("https://www.ecb.europa.eu/press/calendars/mgcgc/html/index.en.html"),
                Some("official_schedule"),
                None,
            ),
        );
    }

    #[test]
    fn event_id_uses_the_ecb_local_calendar_date() {
        assert_eq!(
            target_date(&target("ecb", "2099-04-08", "架空の会合")),
            NaiveDate::from_ymd_opt(2099, 4, 8),
        );
        assert_eq!(
            target_date(&target("ecb", "2099-02-29", "架空の会合")),
            None
        );
    }

    #[test]
    fn only_a_title_with_the_official_decision_day_marker_can_match_a_document() {
        assert_eq!(
            [
                is_decision_day("... monetary policy meeting (Day 1)"),
                is_decision_day(
                    "... monetary policy meeting (Day 2), followed by press conference"
                ),
                is_decision_day("... monetary policy meeting"),
            ],
            [false, true, false],
        );
    }

    #[test]
    fn absent_official_schedule_entry_uses_the_archive_listing() {
        let reference = listing_reference(&target("ecb", "2099-04-08", "架空の政策会合"), false);

        assert_eq!(
            (
                reference.kind,
                reference.url.as_deref(),
                reference.target_period_type.as_deref(),
            ),
            (
                CalendarEventReferenceKind::OfficialListing,
                Some("https://www.ecb.europa.eu/press/govcdec/mopo/html/index.en.html"),
                Some("official_archive"),
            ),
        );
    }
}
