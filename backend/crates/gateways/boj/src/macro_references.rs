mod parser;

use std::collections::{HashMap, HashSet};

use async_trait::async_trait;
use chrono::{Datelike, NaiveDate, Utc};
use chrono_tz::Asia::Tokyo;
use core_application::{
    calendar::macro_reference_targets::MacroReferenceTarget,
    macro_reference_source::{
        MacroOfficialReference, MacroReferenceFetchResult, MacroReferenceSource,
        MacroReferenceSourceError,
    },
};
use core_domain::calendar_event::CalendarEventReferenceKind;

use crate::official_http::OfficialHttpClient;

const SOURCE: &str = "boj";
const STATEMENT_ARCHIVE_URL: &str = "https://www.boj.or.jp/en/mopo/mpmdeci/state_all/index.htm";
const TANKAN_ARCHIVE_URL: &str = "https://www.boj.or.jp/en/statistics/tk/gaiyo/index.htm";
const MEETING_SCHEDULE_URL: &str = "https://www.boj.or.jp/mopo/mpmsche_minu/index.htm";
const RELEASE_SCHEDULE_URL: &str = "https://www.boj.or.jp/about/calendar/index.htm";

pub struct BojMacroReferenceSource {
    http: OfficialHttpClient,
}

impl BojMacroReferenceSource {
    pub fn new(redis_url: &str) -> Result<Self, String> {
        Ok(Self {
            http: OfficialHttpClient::new(redis_url)?,
        })
    }

    async fn statement_candidates(
        &self,
        targets: &[MacroReferenceTarget],
    ) -> Result<HashMap<String, Vec<parser::StatementEntry>>, MacroReferenceSourceError> {
        let needed_years = targets
            .iter()
            .filter_map(|target| target_date(target, "meeting-"))
            .map(|date| date.year())
            .collect::<HashSet<_>>();
        if needed_years.is_empty() {
            return Ok(HashMap::new());
        }

        let index_html = self
            .http
            .get_text(STATEMENT_ARCHIVE_URL)
            .await
            .map_err(source_error)?;
        let pages = parser::statement_archive_pages(&index_html, STATEMENT_ARCHIVE_URL)
            .map_err(source_error)?;
        let mut candidates = HashMap::new();
        for year in needed_years {
            let Some(page_url) = pages.get(&year) else {
                continue;
            };
            let html = self.http.get_text(page_url).await.map_err(source_error)?;
            let page_candidates =
                parser::statement_entries(&html, page_url, year).map_err(source_error)?;
            for target in targets.iter().filter(|target| {
                target_date(target, "meeting-").is_some_and(|date| date.year() == year)
            }) {
                if let Some(date) = target_date(target, "meeting-") {
                    candidates.insert(
                        target.event_external_id.clone(),
                        page_candidates
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

    async fn tankan_candidates(
        &self,
        targets: &[MacroReferenceTarget],
    ) -> Result<HashMap<String, Vec<parser::TankanEntry>>, MacroReferenceSourceError> {
        let needed_years = targets
            .iter()
            .filter_map(|target| target_date(target, "tankan-"))
            .map(|date| date.year())
            .collect::<HashSet<_>>();
        if needed_years.is_empty() {
            return Ok(HashMap::new());
        }

        let index_html = self
            .http
            .get_text(TANKAN_ARCHIVE_URL)
            .await
            .map_err(source_error)?;
        let pages =
            parser::tankan_archive_pages(&index_html, TANKAN_ARCHIVE_URL).map_err(source_error)?;
        let mut candidates = HashMap::new();
        for year in needed_years {
            let Some(page_url) = pages
                .iter()
                .find(|page| page.first_year <= year && year <= page.last_year)
                .map(|page| page.url.as_str())
            else {
                continue;
            };
            let html = self.http.get_text(page_url).await.map_err(source_error)?;
            let page_candidates = parser::tankan_entries(&html, page_url).map_err(source_error)?;
            for target in targets.iter().filter(|target| {
                target_date(target, "tankan-").is_some_and(|date| date.year() == year)
            }) {
                if let Some(date) = target_date(target, "tankan-") {
                    candidates.insert(
                        target.event_external_id.clone(),
                        page_candidates
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
impl MacroReferenceSource for BojMacroReferenceSource {
    fn supports(&self, target: &MacroReferenceTarget) -> bool {
        target.event_source == SOURCE
            && (target_date(target, "meeting-").is_some()
                || target_date(target, "tankan-").is_some())
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
        let meeting_targets = supported
            .iter()
            .filter(|target| target.event_external_id.starts_with("meeting-"))
            .filter(|target| is_decision_day(&target.event_title))
            .cloned()
            .collect::<Vec<_>>();
        let meeting_calendar_targets = supported
            .iter()
            .filter(|target| target.event_external_id.starts_with("meeting-"))
            .collect::<Vec<_>>();
        let tankan_targets = supported
            .iter()
            .filter(|target| target.event_external_id.starts_with("tankan-"))
            .cloned()
            .collect::<Vec<_>>();
        let statements = self.statement_candidates(&meeting_targets).await?;
        let tankan = self.tankan_candidates(&tankan_targets).await?;
        let scheduled_meeting_dates = if meeting_calendar_targets.is_empty() {
            HashSet::new()
        } else {
            let html = self
                .http
                .get_text(MEETING_SCHEDULE_URL)
                .await
                .map_err(source_error)?;
            crate::parser::scheduled_meeting_dates(&html)
                .map_err(|error| source_error(error.to_string()))?
        };
        let scheduled_tankan_dates = if tankan_targets.is_empty() {
            HashSet::new()
        } else {
            let html = self
                .http
                .get_text(RELEASE_SCHEDULE_URL)
                .await
                .map_err(source_error)?;
            let today = Utc::now().with_timezone(&Tokyo).date_naive();
            crate::parser::scheduled_tankan_dates(&html, today)
                .map_err(|error| source_error(error.to_string()))?
        };

        Ok(supported
            .into_iter()
            .map(|target| {
                let references = if target.event_external_id.starts_with("meeting-") {
                    statements
                        .get(&target.event_external_id)
                        .filter(|entries| entries.len() == 1)
                        .and_then(|entries| entries.first())
                        .map(|entry| vec![statement_reference(&target, entry)])
                        .unwrap_or_else(|| {
                            let scheduled = target_date(&target, "meeting-")
                                .is_some_and(|date| scheduled_meeting_dates.contains(&date));
                            vec![listing_reference(&target, false, scheduled)]
                        })
                } else {
                    tankan
                        .get(&target.event_external_id)
                        .filter(|entries| entries.len() == 1)
                        .and_then(|entries| entries.first())
                        .map(|entry| vec![tankan_reference(&target, entry)])
                        .unwrap_or_else(|| {
                            let scheduled = target_date(&target, "tankan-")
                                .is_some_and(|date| scheduled_tankan_dates.contains(&date));
                            vec![listing_reference(&target, true, scheduled)]
                        })
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
            "公式資料一覧に公表日 {} と資料名「{}」を確認",
            entry.publication_date, entry.title
        )),
        publication_evidence_url: Some(entry.archive_url.clone()),
    }
}

fn tankan_reference(
    target: &MacroReferenceTarget,
    entry: &parser::TankanEntry,
) -> MacroOfficialReference {
    MacroOfficialReference {
        target: target.clone(),
        kind: CalendarEventReferenceKind::IndividualDocument,
        source: SOURCE.to_owned(),
        external_id: entry.url.clone(),
        title: entry.title.clone(),
        url: Some(entry.url.clone()),
        target_period_start: Some(entry.survey_period_start),
        target_period_end: Some(entry.survey_period_end),
        target_period_type: Some("tankan".to_owned()),
        publication_evidence: Some(format!(
            "公式 TANKAN (Summary) 一覧に公表日 {} と資料名「{}」を確認",
            entry.publication_date, entry.survey_title
        )),
        publication_evidence_url: Some(entry.archive_url.clone()),
    }
}

fn listing_reference(
    target: &MacroReferenceTarget,
    is_tankan: bool,
    scheduled: bool,
) -> MacroOfficialReference {
    let (url, title) = if scheduled && is_tankan {
        (RELEASE_SCHEDULE_URL, "短観の公式日程")
    } else if scheduled {
        (MEETING_SCHEDULE_URL, "金融政策決定会合の公式日程")
    } else if is_tankan {
        (TANKAN_ARCHIVE_URL, "短観の公式資料一覧")
    } else {
        (STATEMENT_ARCHIVE_URL, "金融政策決定資料の公式一覧")
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

fn target_date(target: &MacroReferenceTarget, prefix: &str) -> Option<NaiveDate> {
    target
        .event_external_id
        .strip_prefix(prefix)
        .and_then(|value| NaiveDate::parse_from_str(value, "%Y-%m-%d").ok())
}

fn is_decision_day(title: &str) -> bool {
    title
        .strip_prefix("金融政策決定会合 (")
        .and_then(|value| value.strip_suffix("日目)"))
        .and_then(|value| value.parse::<u32>().ok())
        .is_some_and(|day| day > 1)
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
    fn first_meeting_day_has_an_official_listing_instead_of_a_decision_document() {
        let target = target("boj", "meeting-2099-04-08", "金融政策決定会合 (1日目)");

        let actual = listing_reference(&target, false, true);

        assert_eq!(
            (
                actual.kind,
                actual.url.as_deref(),
                actual.target_period_type.as_deref(),
                actual.publication_evidence.as_deref(),
            ),
            (
                CalendarEventReferenceKind::OfficialListing,
                Some("https://www.boj.or.jp/mopo/mpmsche_minu/index.htm"),
                Some("official_schedule"),
                None,
            ),
        );
    }

    #[test]
    fn event_dates_must_be_real_iso_calendar_dates() {
        assert_eq!(
            target_date(
                &target("boj", "meeting-2099-02-29", "架空の会合"),
                "meeting-"
            ),
            None,
        );
        assert_eq!(
            target_date(
                &target("boj", "meeting-2099-04-08", "架空の会合"),
                "meeting-"
            ),
            NaiveDate::from_ymd_opt(2099, 4, 8),
        );
    }

    #[test]
    fn only_an_explicit_nonfirst_meeting_day_can_have_a_decision_document() {
        assert_eq!(
            [
                is_decision_day("金融政策決定会合 (1日目)"),
                is_decision_day("金融政策決定会合 (2日目)"),
                is_decision_day("金融政策決定会合 (x日目)"),
                is_decision_day("金融政策決定会合"),
            ],
            [false, true, false, false],
        );
    }
}
