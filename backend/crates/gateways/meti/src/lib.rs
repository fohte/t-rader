use std::collections::{HashMap, HashSet};

use crate::official_http::OfficialHttpClient;
use async_trait::async_trait;
use chrono::{Datelike, Months, NaiveDate};
use core_application::{
    calendar::macro_reference_targets::MacroReferenceTarget,
    macro_reference_source::{
        MacroOfficialReference, MacroReferenceFetchResult, MacroReferenceSource,
        MacroReferenceSourceError,
    },
};
use core_domain::calendar_event::CalendarEventReferenceKind;

mod official_http;
mod parser;

const IIP_ARCHIVE_URL: &str = "https://www.meti.go.jp/statistics/tyo/iip/kako_press.html";
const IIP_STAT_CODE: &str = "00550300";
const IIP_CANDIDATE_PERIOD_MONTHS_BEFORE_RELEASE: [u32; 2] = [1, 2];

pub struct MetiMacroReferenceSource {
    http: OfficialHttpClient,
}

impl MetiMacroReferenceSource {
    pub fn new(redis_url: &str) -> Result<Self, String> {
        let http = OfficialHttpClient::new(redis_url).map_err(|error| error.to_string())?;
        Ok(Self { http })
    }
}

#[async_trait]
impl MacroReferenceSource for MetiMacroReferenceSource {
    fn supports(&self, target: &MacroReferenceTarget) -> bool {
        target.event_source == "e_stat"
            && parse_publication_date(&target.event_external_id).is_some()
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

        let archive = match self.http.get_text_cached(IIP_ARCHIVE_URL).await {
            Ok(archive) => archive,
            Err(error) => {
                let failure = error.to_string();
                return Ok(supported_targets
                    .into_iter()
                    .map(|target| MacroReferenceFetchResult::retry(target, failure.clone()))
                    .collect());
            }
        };
        let Some(release_pages) = parser::parse_iip_archive(&archive, IIP_ARCHIVE_URL) else {
            return Ok(supported_targets
                .into_iter()
                .map(|target| {
                    MacroReferenceFetchResult::retry(
                        target,
                        "industrial production archive response could not be parsed",
                    )
                })
                .collect());
        };
        let candidate_periods = supported_targets
            .iter()
            .filter_map(|target| parse_publication_date(&target.event_external_id))
            .flat_map(iip_candidate_periods)
            .collect::<HashSet<_>>();

        let mut references_by_target = HashMap::<String, Vec<MacroOfficialReference>>::new();
        let mut ambiguous_targets = HashSet::<String>::new();
        for release_page in release_pages
            .into_iter()
            .filter(|page| candidate_periods.contains(&page.period_start))
        {
            let html = match self.http.get_text_cached(&release_page.url).await {
                Ok(html) => html,
                Err(error) => {
                    let failure = error.to_string();
                    return Ok(supported_targets
                        .into_iter()
                        .map(|target| MacroReferenceFetchResult::retry(target, failure.clone()))
                        .collect());
                }
            };
            let actual_publication_date = parser::parse_iip_publication_date(&html);
            for target in &supported_targets {
                let Some(target_date) = parse_publication_date(&target.event_external_id) else {
                    continue;
                };
                let Some(actual_publication_date) = actual_publication_date else {
                    ambiguous_targets.insert(target.event_external_id.clone());
                    continue;
                };
                if actual_publication_date == target_date {
                    if let Some(reference) = parser::parse_iip_release_page(
                        &html,
                        &release_page.url,
                        target,
                        IIP_STAT_CODE,
                    ) {
                        if reference.target_period_start == Some(release_page.period_start) {
                            references_by_target
                                .entry(target.event_external_id.clone())
                                .or_default()
                                .push(reference);
                        } else {
                            ambiguous_targets.insert(target.event_external_id.clone());
                        }
                    } else {
                        ambiguous_targets.insert(target.event_external_id.clone());
                    }
                }
            }
        }

        Ok(supported_targets
            .into_iter()
            .map(|target| {
                let references = references_by_target
                    .remove(&target.event_external_id)
                    .unwrap_or_default();
                if ambiguous_targets.contains(&target.event_external_id) {
                    listing_result(&target)
                } else {
                    individual_or_listing(target, references)
                }
            })
            .collect())
    }
}

fn iip_candidate_periods(publication_date: NaiveDate) -> Vec<NaiveDate> {
    let Some(release_month) =
        NaiveDate::from_ymd_opt(publication_date.year(), publication_date.month(), 1)
    else {
        return Vec::new();
    };
    IIP_CANDIDATE_PERIOD_MONTHS_BEFORE_RELEASE
        .into_iter()
        .filter_map(|months| release_month.checked_sub_months(Months::new(months)))
        .collect()
}

fn parse_publication_date(external_id: &str) -> Option<chrono::NaiveDate> {
    let (stat_code, published_at) = external_id.split_once(':')?;
    if stat_code != IIP_STAT_CODE || published_at.len() != 12 {
        return None;
    }
    Some(
        chrono::NaiveDateTime::parse_from_str(published_at, "%Y%m%d%H%M")
            .ok()?
            .date(),
    )
}

fn listing_result(target: &MacroReferenceTarget) -> MacroReferenceFetchResult {
    MacroReferenceFetchResult::success(
        target.clone(),
        vec![MacroOfficialReference {
            target: target.clone(),
            kind: CalendarEventReferenceKind::OfficialListing,
            source: "meti_jp".to_owned(),
            external_id: format!("iip-release-listing:{}", target.event_external_id),
            title: "公式統計資料一覧".to_owned(),
            url: Some(IIP_ARCHIVE_URL.to_owned()),
            target_period_start: None,
            target_period_end: None,
            target_period_type: None,
            publication_evidence: None,
            publication_evidence_url: None,
        }],
    )
}

fn individual_or_listing(
    target: MacroReferenceTarget,
    references: Vec<MacroOfficialReference>,
) -> MacroReferenceFetchResult {
    if target.same_day_event_count == 1 && references.len() == 1 {
        MacroReferenceFetchResult::success(target, references)
    } else {
        listing_result(&target)
    }
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;
    use core_application::{
        calendar::macro_reference_targets::MacroReferenceTarget,
        macro_reference_source::{MacroOfficialReference, MacroReferenceFetchResult},
    };
    use core_domain::calendar_event::CalendarEventReferenceKind;

    use super::{iip_candidate_periods, individual_or_listing, listing_result};

    #[test]
    fn candidate_periods_include_the_two_months_before_the_release_date() {
        assert_eq!(
            iip_candidate_periods(NaiveDate::from_ymd_opt(2099, 10, 15).unwrap_or(NaiveDate::MIN)),
            vec![
                NaiveDate::from_ymd_opt(2099, 9, 1).unwrap_or(NaiveDate::MIN),
                NaiveDate::from_ymd_opt(2099, 8, 1).unwrap_or(NaiveDate::MIN),
            ],
        );
    }

    #[test]
    fn individual_link_requires_unique_calendar_and_official_records() {
        let target = MacroReferenceTarget {
            event_source: "e_stat".to_owned(),
            event_external_id: "sample-stat-code:209911150850".to_owned(),
            event_title: "架空の統計イベント".to_owned(),
            same_day_event_count: 1,
        };
        let reference = MacroOfficialReference {
            target: target.clone(),
            kind: CalendarEventReferenceKind::IndividualDocument,
            source: "meti_jp".to_owned(),
            external_id: "https://example.invalid/result/sample.html".to_owned(),
            title: "架空の生産結果".to_owned(),
            url: Some("https://example.invalid/result/sample.html".to_owned()),
            target_period_start: Some(
                NaiveDate::from_ymd_opt(2099, 10, 1).unwrap_or(NaiveDate::MIN),
            ),
            target_period_end: Some(
                NaiveDate::from_ymd_opt(2099, 10, 31).unwrap_or(NaiveDate::MIN),
            ),
            target_period_type: Some("month".to_owned()),
            publication_evidence: Some(
                "Synthetic official record published on 2099-11-15".to_owned(),
            ),
            publication_evidence_url: Some("https://example.invalid/result/sample.html".to_owned()),
        };
        let duplicate_calendar_target = MacroReferenceTarget {
            same_day_event_count: 2,
            ..target.clone()
        };
        let actual = (
            individual_or_listing(target.clone(), vec![reference.clone(), reference.clone()]),
            individual_or_listing(duplicate_calendar_target.clone(), vec![reference.clone()]),
            individual_or_listing(target.clone(), vec![reference.clone()]),
        );

        assert_eq!(
            actual,
            (
                listing_result(&target),
                listing_result(&duplicate_calendar_target),
                MacroReferenceFetchResult::success(target, vec![reference]),
            ),
        );
    }
}
