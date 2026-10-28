mod parser;

use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

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

use crate::official_http::{OfficialHttp, OfficialHttpClient};

const SOURCE: &str = "ecb";
const DECISION_ARCHIVE_URL: &str =
    "https://www.ecb.europa.eu/press/govcdec/mopo/html/index.en.html";
const PREVIOUS_DECISION_ARCHIVE_URL: &str =
    "https://www.ecb.europa.eu/press/govcdec/mopo/previous/html/index.en.html";
const MEETING_SCHEDULE_URL: &str =
    "https://www.ecb.europa.eu/press/calendars/mgcgc/html/index.en.html";

pub struct EcbMacroReferenceSource {
    http: Arc<dyn OfficialHttp>,
}

impl EcbMacroReferenceSource {
    pub fn new(redis_url: &str) -> Result<Self, String> {
        Ok(Self {
            http: Arc::new(OfficialHttpClient::new(redis_url)?),
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

        if supported
            .iter()
            .all(|target| !is_decision_day(&target.event_title))
        {
            return Ok(supported
                .into_iter()
                .map(|target| {
                    let scheduled = calendar_events
                        .iter()
                        .any(|event| event.external_id == target.event_external_id);
                    let references = if scheduled {
                        vec![listing_reference(&target, true)]
                    } else {
                        Vec::new()
                    };
                    MacroReferenceFetchResult::success(target, references)
                })
                .collect());
        }

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
                    if scheduled || is_decision_day(&target.event_title) {
                        vec![listing_reference(&target, scheduled)]
                    } else {
                        Vec::new()
                    }
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
    use std::{collections::HashMap, sync::Arc};

    use async_trait::async_trait;
    use chrono::NaiveDate;
    use core_application::{
        calendar::macro_reference_targets::MacroReferenceTarget,
        macro_reference_source::{
            MacroOfficialReference, MacroReferenceFetchResult, MacroReferenceSource,
        },
    };
    use core_domain::calendar_event::CalendarEventReferenceKind;

    use crate::official_http::OfficialHttp;

    use super::{
        DECISION_ARCHIVE_URL, EcbMacroReferenceSource, MEETING_SCHEDULE_URL, is_decision_day,
        listing_reference, target_date,
    };

    struct FixtureHttp(HashMap<&'static str, &'static str>);

    #[async_trait]
    impl OfficialHttp for FixtureHttp {
        async fn get_text(&self, url: &str) -> Result<String, String> {
            self.0
                .get(url)
                .map(|html| (*html).to_owned())
                .ok_or_else(|| "synthetic response is missing".to_owned())
        }
    }

    fn fixture_http(responses: &[(&'static str, &'static str)]) -> Arc<dyn OfficialHttp> {
        Arc::new(FixtureHttp(responses.iter().copied().collect()))
    }

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
    fn decision_day_without_an_official_schedule_uses_the_archive_listing() {
        let reference = listing_reference(
            &target(
                "ecb",
                "2099-04-08",
                "Governing Council of the ECB: monetary policy meeting (Day 2), followed by press conference",
            ),
            false,
        );

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

    #[tokio::test]
    async fn source_links_a_decision_document_only_on_the_scheduled_decision_day() {
        let first_day = target(
            "ecb",
            "2099-04-07",
            "Governing Council of the ECB: monetary policy meeting (Day 1)",
        );
        let decision_day = target(
            "ecb",
            "2099-04-08",
            "Governing Council of the ECB: monetary policy meeting (Day 2), followed by press conference",
        );
        let source = EcbMacroReferenceSource {
            http: fixture_http(&[
                (
                    MEETING_SCHEDULE_URL,
                    include_str!("fixtures/meeting_schedule_reference_source_synthetic.html"),
                ),
                (
                    DECISION_ARCHIVE_URL,
                    include_str!("fixtures/decision_archive_index_synthetic.html"),
                ),
                (
                    "https://www.ecb.europa.eu/press/govcdec/mopo/2099/html/index_include.en.html",
                    include_str!("fixtures/decision_archive_synthetic.html"),
                ),
            ]),
        };

        let actual = source
            .fetch_references(&[first_day.clone(), decision_day.clone()])
            .await
            .expect("synthetic archive responses should resolve");

        let document_url = "https://www.ecb.europa.eu/press/fictional/decision-document.html";
        assert_eq!(
            actual,
            vec![
                MacroReferenceFetchResult::success(
                    first_day.clone(),
                    vec![MacroOfficialReference {
                        target: first_day,
                        kind: CalendarEventReferenceKind::OfficialListing,
                        source: "ecb".to_owned(),
                        external_id: "2099-04-07:official-listing".to_owned(),
                        title: "ECB の公式会合日程".to_owned(),
                        url: Some(MEETING_SCHEDULE_URL.to_owned()),
                        target_period_start: None,
                        target_period_end: None,
                        target_period_type: Some("official_schedule".to_owned()),
                        publication_evidence: None,
                        publication_evidence_url: None,
                    }],
                ),
                MacroReferenceFetchResult::success(
                    decision_day.clone(),
                    vec![MacroOfficialReference {
                        target: decision_day,
                        kind: CalendarEventReferenceKind::IndividualDocument,
                        source: "ecb".to_owned(),
                        external_id: document_url.to_owned(),
                        title: "Monetary policy decisions".to_owned(),
                        url: Some(document_url.to_owned()),
                        target_period_start: NaiveDate::from_ymd_opt(2099, 4, 8),
                        target_period_end: NaiveDate::from_ymd_opt(2099, 4, 8),
                        target_period_type: Some("meeting".to_owned()),
                        publication_evidence: Some(
                            "公式アーカイブに公表日 2099-04-08 と資料名「Monetary policy decisions」を確認"
                                .to_owned(),
                        ),
                        publication_evidence_url: Some(
                            "https://www.ecb.europa.eu/press/govcdec/mopo/2099/html/index_include.en.html"
                                .to_owned(),
                        ),
                    }],
                ),
            ],
        );
    }

    #[tokio::test]
    async fn source_keeps_an_unscheduled_first_meeting_day_unlinked_without_loading_decision_archives()
     {
        let first_day = target(
            "ecb",
            "2099-04-06",
            "Governing Council of the ECB: monetary policy meeting (Day 1)",
        );
        let source = EcbMacroReferenceSource {
            http: fixture_http(&[(
                MEETING_SCHEDULE_URL,
                include_str!("fixtures/meeting_schedule_reference_source_synthetic.html"),
            )]),
        };

        let actual = source
            .fetch_references(std::slice::from_ref(&first_day))
            .await
            .expect("the meeting schedule fixture should resolve");

        assert_eq!(
            actual,
            vec![MacroReferenceFetchResult::success(first_day, vec![])],
        );
    }

    #[tokio::test]
    async fn source_keeps_ambiguous_decision_records_as_an_official_listing() {
        let decision_day = target(
            "ecb",
            "2099-04-08",
            "Governing Council of the ECB: monetary policy meeting (Day 2), followed by press conference",
        );
        let source = EcbMacroReferenceSource {
            http: fixture_http(&[
                (
                    MEETING_SCHEDULE_URL,
                    include_str!("fixtures/meeting_schedule_reference_source_synthetic.html"),
                ),
                (
                    DECISION_ARCHIVE_URL,
                    include_str!("fixtures/decision_archive_index_synthetic.html"),
                ),
                (
                    "https://www.ecb.europa.eu/press/govcdec/mopo/2099/html/index_include.en.html",
                    include_str!("fixtures/decision_archive_ambiguous_synthetic.html"),
                ),
            ]),
        };

        let actual = source
            .fetch_references(std::slice::from_ref(&decision_day))
            .await
            .expect("synthetic archive responses should resolve");

        assert_eq!(
            actual,
            vec![MacroReferenceFetchResult::success(
                decision_day.clone(),
                vec![MacroOfficialReference {
                    target: decision_day,
                    kind: CalendarEventReferenceKind::OfficialListing,
                    source: "ecb".to_owned(),
                    external_id: "2099-04-08:official-listing".to_owned(),
                    title: "ECB の公式会合日程".to_owned(),
                    url: Some(MEETING_SCHEDULE_URL.to_owned()),
                    target_period_start: None,
                    target_period_end: None,
                    target_period_type: Some("official_schedule".to_owned()),
                    publication_evidence: None,
                    publication_evidence_url: None,
                }],
            )],
        );
    }
}
