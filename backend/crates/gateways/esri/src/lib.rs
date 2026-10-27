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

const GDP_STAT_CODE: &str = "00100409";
const MACHINERY_STAT_CODE: &str = "00100401";
const GDP_LISTING_URL: &str = "https://www.esri.cao.go.jp/jp/sna/menu.html";
const MACHINERY_INDEX_URL: &str = "https://www.esri.cao.go.jp/jp/stat/juchu/juchu.html";

pub struct EsriMacroReferenceSource {
    http: OfficialHttpClient,
}

impl EsriMacroReferenceSource {
    pub fn new(redis_url: &str) -> Result<Self, String> {
        let http = OfficialHttpClient::new(redis_url).map_err(|error| error.to_string())?;
        Ok(Self { http })
    }

    async fn fetch_gdp(
        &self,
        targets: Vec<MacroReferenceTarget>,
    ) -> Vec<MacroReferenceFetchResult> {
        let mut targets_by_year = HashMap::<i32, Vec<MacroReferenceTarget>>::new();
        for target in targets {
            if let Some((_, published_at)) = parse_estat_id(&target.event_external_id) {
                targets_by_year
                    .entry(published_at.year())
                    .or_default()
                    .push(target);
            }
        }

        let mut results = Vec::new();
        for (year, year_targets) in targets_by_year {
            let archive_url = parser::gdp_archive_url(year);
            let html =
                match self.http.get_text_cached(&archive_url).await {
                    Ok(html) => html,
                    Err(error) => {
                        let failure = error.to_string();
                        results.extend(year_targets.into_iter().map(|target| {
                            MacroReferenceFetchResult::retry(target, failure.clone())
                        }));
                        continue;
                    }
                };
            for target in year_targets {
                let references =
                    parser::parse_gdp_archive(&html, &archive_url, &target, GDP_STAT_CODE)
                        .unwrap_or_default();
                results.push(individual_or_listing(
                    target,
                    references,
                    "gdp-release-listing",
                    GDP_LISTING_URL,
                ));
            }
        }
        results
    }

    async fn fetch_machinery(
        &self,
        targets: Vec<MacroReferenceTarget>,
    ) -> Vec<MacroReferenceFetchResult> {
        if targets.is_empty() {
            return Vec::new();
        }
        let html = match self.http.get_text_cached(MACHINERY_INDEX_URL).await {
            Ok(html) => html,
            Err(error) => {
                let failure = error.to_string();
                return targets
                    .into_iter()
                    .map(|target| MacroReferenceFetchResult::retry(target, failure.clone()))
                    .collect();
            }
        };
        let Some(release_pages) = parser::parse_machinery_index(&html, MACHINERY_INDEX_URL) else {
            return targets
                .into_iter()
                .map(|target| {
                    MacroReferenceFetchResult::retry(
                        target,
                        "machinery orders index response could not be parsed",
                    )
                })
                .collect();
        };
        let candidate_periods = targets
            .iter()
            .filter_map(|target| parse_estat_id(&target.event_external_id))
            .flat_map(|(_, published_at)| machinery_candidate_periods(published_at.date()))
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
                    return targets
                        .into_iter()
                        .map(|target| MacroReferenceFetchResult::retry(target, failure.clone()))
                        .collect();
                }
            };
            let actual_publication_date = parser::parse_machinery_publication_date(&html);
            for target in &targets {
                let Some((_, published_at)) = parse_estat_id(&target.event_external_id) else {
                    continue;
                };
                let Some(actual_publication_date) = actual_publication_date else {
                    ambiguous_targets.insert(target.event_external_id.clone());
                    continue;
                };
                if actual_publication_date == published_at.date() {
                    if let Some(reference) = parser::parse_machinery_release_page(
                        &html,
                        &release_page.url,
                        target,
                        MACHINERY_STAT_CODE,
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

        targets
            .into_iter()
            .map(|target| {
                let references = references_by_target
                    .remove(&target.event_external_id)
                    .unwrap_or_default();
                if ambiguous_targets.contains(&target.event_external_id) {
                    listing_result(
                        &target,
                        "machinery-orders-listing",
                        "公式統計資料一覧",
                        MACHINERY_INDEX_URL,
                    )
                } else {
                    individual_or_listing(
                        target,
                        references,
                        "machinery-orders-listing",
                        MACHINERY_INDEX_URL,
                    )
                }
            })
            .collect()
    }
}

fn machinery_candidate_periods(publication_date: NaiveDate) -> Vec<NaiveDate> {
    let Some(release_month) =
        NaiveDate::from_ymd_opt(publication_date.year(), publication_date.month(), 1)
    else {
        return Vec::new();
    };
    [1, 2]
        .into_iter()
        .filter_map(|months| release_month.checked_sub_months(Months::new(months)))
        .collect()
}

#[async_trait]
impl MacroReferenceSource for EsriMacroReferenceSource {
    fn supports(&self, target: &MacroReferenceTarget) -> bool {
        target.event_source == "e_stat"
            && parse_estat_id(&target.event_external_id)
                .is_some_and(|(code, _)| matches!(code, GDP_STAT_CODE | MACHINERY_STAT_CODE))
    }

    async fn fetch_references(
        &self,
        targets: &[MacroReferenceTarget],
    ) -> Result<Vec<MacroReferenceFetchResult>, MacroReferenceSourceError> {
        let mut gdp_targets = Vec::new();
        let mut machinery_targets = Vec::new();
        for target in targets.iter().filter(|target| self.supports(target)) {
            match parse_estat_id(&target.event_external_id).map(|(code, _)| code) {
                Some(GDP_STAT_CODE) => gdp_targets.push(target.clone()),
                Some(MACHINERY_STAT_CODE) => machinery_targets.push(target.clone()),
                _ => {}
            }
        }
        let mut results = self.fetch_gdp(gdp_targets).await;
        results.extend(self.fetch_machinery(machinery_targets).await);
        Ok(results)
    }
}

fn parse_estat_id(external_id: &str) -> Option<(&str, chrono::NaiveDateTime)> {
    let (code, published_at) = external_id.split_once(':')?;
    if !matches!(code, GDP_STAT_CODE | MACHINERY_STAT_CODE) || published_at.len() != 12 {
        return None;
    }
    Some((
        code,
        chrono::NaiveDateTime::parse_from_str(published_at, "%Y%m%d%H%M").ok()?,
    ))
}

fn individual_or_listing(
    target: MacroReferenceTarget,
    references: Vec<MacroOfficialReference>,
    external_id: &str,
    url: &str,
) -> MacroReferenceFetchResult {
    if target.same_day_event_count == 1 && references.len() == 1 {
        MacroReferenceFetchResult::success(target, references)
    } else {
        listing_result(&target, external_id, "公式統計資料一覧", url)
    }
}

fn listing_result(
    target: &MacroReferenceTarget,
    external_id: &str,
    title: &str,
    url: &str,
) -> MacroReferenceFetchResult {
    MacroReferenceFetchResult::success(
        target.clone(),
        vec![MacroOfficialReference {
            target: target.clone(),
            kind: CalendarEventReferenceKind::OfficialListing,
            source: "esri_jp".to_owned(),
            external_id: format!("{external_id}:{}", target.event_external_id),
            title: title.to_owned(),
            url: Some(url.to_owned()),
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
    use chrono::NaiveDate;
    use core_application::{
        calendar::macro_reference_targets::MacroReferenceTarget,
        macro_reference_source::{MacroOfficialReference, MacroReferenceFetchResult},
    };
    use core_domain::calendar_event::CalendarEventReferenceKind;

    use super::{individual_or_listing, machinery_candidate_periods};

    #[test]
    fn machinery_candidates_include_the_two_months_before_the_release_date() {
        assert_eq!(
            machinery_candidate_periods(
                NaiveDate::from_ymd_opt(2099, 10, 15).unwrap_or(NaiveDate::MIN),
            ),
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
            event_external_id: "sample-stat-code:209904080850".to_owned(),
            event_title: "架空の統計イベント".to_owned(),
            same_day_event_count: 1,
        };
        let reference = MacroOfficialReference {
            target: target.clone(),
            kind: CalendarEventReferenceKind::IndividualDocument,
            source: "esri_jp".to_owned(),
            external_id: "https://example.invalid/result/sample.pdf".to_owned(),
            title: "架空の統計資料".to_owned(),
            url: Some("https://example.invalid/result/sample.pdf".to_owned()),
            target_period_start: Some(
                NaiveDate::from_ymd_opt(2099, 1, 1).unwrap_or(NaiveDate::MIN),
            ),
            target_period_end: Some(NaiveDate::from_ymd_opt(2099, 3, 31).unwrap_or(NaiveDate::MIN)),
            target_period_type: Some("quarter".to_owned()),
            publication_evidence: Some(
                "Synthetic official record published on 2099-04-08".to_owned(),
            ),
            publication_evidence_url: Some("https://example.invalid/result/sample.pdf".to_owned()),
        };
        let duplicate_calendar_target = MacroReferenceTarget {
            same_day_event_count: 2,
            ..target.clone()
        };
        let actual = (
            individual_or_listing(
                target.clone(),
                vec![reference.clone(), reference.clone()],
                "sample-listing",
                "https://example.invalid/listing",
            ),
            individual_or_listing(
                duplicate_calendar_target.clone(),
                vec![reference.clone()],
                "sample-listing",
                "https://example.invalid/listing",
            ),
            individual_or_listing(
                target.clone(),
                vec![reference.clone()],
                "sample-listing",
                "https://example.invalid/listing",
            ),
        );

        assert_eq!(
            actual,
            (
                super::listing_result(
                    &target,
                    "sample-listing",
                    "公式統計資料一覧",
                    "https://example.invalid/listing",
                ),
                super::listing_result(
                    &duplicate_calendar_target,
                    "sample-listing",
                    "公式統計資料一覧",
                    "https://example.invalid/listing",
                ),
                MacroReferenceFetchResult::success(target, vec![reference]),
            ),
        );
    }
}
