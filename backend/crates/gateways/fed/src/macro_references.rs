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

const SOURCE: &str = "fed";
const CALENDAR_URL: &str = "https://www.federalreserve.gov/monetarypolicy/fomccalendars.htm";
const HISTORICAL_INDEX_URL: &str =
    "https://www.federalreserve.gov/monetarypolicy/fomc_historical_year.htm";
const HISTORICAL_LISTING_URL: &str =
    "https://www.federalreserve.gov/monetarypolicy/fomc_historical.htm";

pub struct FedMacroReferenceSource {
    http: Arc<dyn OfficialHttp>,
}

impl FedMacroReferenceSource {
    pub fn new(redis_url: &str) -> Result<Self, String> {
        Ok(Self {
            http: Arc::new(OfficialHttpClient::new(redis_url)?),
        })
    }

    async fn archive_candidates(
        &self,
        unresolved: &[MacroReferenceTarget],
    ) -> Result<HashMap<String, Vec<parser::StatementEntry>>, MacroReferenceSourceError> {
        if unresolved.is_empty() {
            return Ok(HashMap::new());
        }
        let years = unresolved
            .iter()
            .filter_map(target_date)
            .map(|date| date.year())
            .collect::<HashSet<_>>();
        let index_html = self
            .http
            .get_text(HISTORICAL_INDEX_URL)
            .await
            .map_err(source_error)?;
        let pages = parser::historical_year_pages(&index_html, HISTORICAL_INDEX_URL)
            .map_err(source_error)?;
        let mut candidates = HashMap::new();
        for year in years {
            let Some(page_url) = pages.get(&year) else {
                continue;
            };
            let html = self.http.get_text(page_url).await.map_err(source_error)?;
            let entries = parser::historical_statement_entries(&html, page_url, year)
                .map_err(source_error)?;
            for target in unresolved
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
                            .collect(),
                    );
                }
            }
        }
        Ok(candidates)
    }
}

#[async_trait]
impl MacroReferenceSource for FedMacroReferenceSource {
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
            .get_text(CALENDAR_URL)
            .await
            .map_err(source_error)?;
        let calendar_entries = parser::calendar_statement_entries(&calendar_html, CALENDAR_URL)
            .map_err(source_error)?;
        let scheduled_meeting_dates =
            parser::scheduled_meeting_dates(&calendar_html).map_err(source_error)?;
        let mut candidates = HashMap::<String, Vec<parser::StatementEntry>>::new();
        for target in &supported {
            if let Some(date) = target_date(target) {
                candidates.insert(
                    target.event_external_id.clone(),
                    calendar_entries
                        .iter()
                        .filter(|entry| entry.publication_date == date)
                        .cloned()
                        .collect(),
                );
            }
        }
        let unresolved = supported
            .iter()
            .filter(|target| {
                candidates
                    .get(&target.event_external_id)
                    .is_none_or(Vec::is_empty)
            })
            .cloned()
            .collect::<Vec<_>>();
        let archive_entries = self.archive_candidates(&unresolved).await?;

        Ok(supported
            .into_iter()
            .map(|target| {
                let mut entries = candidates
                    .remove(&target.event_external_id)
                    .unwrap_or_default();
                entries.extend(
                    archive_entries
                        .get(&target.event_external_id)
                        .into_iter()
                        .flatten()
                        .cloned(),
                );
                entries.sort_by(|left, right| left.url.cmp(&right.url));
                entries.dedup_by(|left, right| left.url == right.url);
                let references = if entries.len() == 1 {
                    entries
                        .first()
                        .map(|entry| vec![statement_reference(&target, entry)])
                        .unwrap_or_default()
                } else {
                    let scheduled = target_date(&target)
                        .is_some_and(|date| scheduled_meeting_dates.contains(&date));
                    vec![listing_reference(&target, scheduled)]
                };
                MacroReferenceFetchResult::success(target, references)
            })
            .collect())
    }
}

fn statement_reference(
    target: &MacroReferenceTarget,
    entry: &parser::StatementEntry,
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
            "公式 FOMC カレンダーに会合日 {} と声明のリンクを確認",
            entry.publication_date
        )),
        publication_evidence_url: Some(entry.archive_url.clone()),
    }
}

fn listing_reference(target: &MacroReferenceTarget, scheduled: bool) -> MacroOfficialReference {
    let (url, title) = if scheduled {
        (CALENDAR_URL, "FOMC の公式日程")
    } else {
        (HISTORICAL_LISTING_URL, "FOMC 声明の公式一覧")
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
        CALENDAR_URL, FedMacroReferenceSource, HISTORICAL_INDEX_URL, listing_reference, target_date,
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

    fn target(event_source: &str, external_id: &str) -> MacroReferenceTarget {
        MacroReferenceTarget {
            event_source: event_source.to_owned(),
            event_external_id: external_id.to_owned(),
            event_title: "架空の政策イベント".to_owned(),
            same_day_event_count: 1,
        }
    }

    #[test]
    fn official_listing_keeps_a_schedule_or_archive_period_label() {
        let future = listing_reference(&target("fed", "2099-04-08"), true);

        assert_eq!(
            (
                future.kind,
                future.url.as_deref(),
                future.target_period_type.as_deref(),
            ),
            (
                CalendarEventReferenceKind::OfficialListing,
                Some("https://www.federalreserve.gov/monetarypolicy/fomccalendars.htm"),
                Some("official_schedule"),
            ),
        );
    }

    #[test]
    fn event_id_is_the_new_york_statement_date() {
        assert_eq!(
            target_date(&target("fed", "2099-04-08")),
            NaiveDate::from_ymd_opt(2099, 4, 8),
        );
        assert_eq!(target_date(&target("fed", "invalid")), None);
    }

    #[tokio::test]
    async fn source_matches_saved_statement_events_against_the_official_archive() {
        let event = target("fed", "2099-04-08");
        let source = FedMacroReferenceSource {
            http: fixture_http(&[
                (
                    CALENDAR_URL,
                    include_str!("fixtures/calendar_reference_synthetic.html"),
                ),
                (
                    HISTORICAL_INDEX_URL,
                    include_str!("fixtures/historical_year_index_synthetic.html"),
                ),
                (
                    "https://www.federalreserve.gov/monetarypolicy/fictional-period.html",
                    include_str!("fixtures/historical_archive_synthetic.html"),
                ),
            ]),
        };

        let actual = source
            .fetch_references(std::slice::from_ref(&event))
            .await
            .expect("synthetic archive responses should resolve");

        let document_url =
            "https://www.federalreserve.gov/newsevents/pressreleases/fictional-statement.html";
        assert_eq!(
            actual,
            vec![MacroReferenceFetchResult::success(
                event.clone(),
                vec![MacroOfficialReference {
                    target: event,
                    kind: CalendarEventReferenceKind::IndividualDocument,
                    source: "fed".to_owned(),
                    external_id: document_url.to_owned(),
                    title: "Statement".to_owned(),
                    url: Some(document_url.to_owned()),
                    target_period_start: NaiveDate::from_ymd_opt(2099, 4, 8),
                    target_period_end: NaiveDate::from_ymd_opt(2099, 4, 8),
                    target_period_type: Some("meeting".to_owned()),
                    publication_evidence: Some(
                        "公式 FOMC カレンダーに会合日 2099-04-08 と声明のリンクを確認".to_owned(),
                    ),
                    publication_evidence_url: Some(
                        "https://www.federalreserve.gov/monetarypolicy/fictional-period.html"
                            .to_owned(),
                    ),
                }],
            )],
        );
    }
}
