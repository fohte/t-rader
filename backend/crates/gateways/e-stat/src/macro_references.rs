mod parser;

use std::collections::{HashMap, HashSet};

use async_trait::async_trait;
use chrono::{Datelike, NaiveDate, NaiveDateTime};
use core_application::{
    calendar::macro_reference_targets::MacroReferenceTarget,
    macro_reference_source::{
        MacroOfficialReference, MacroReferenceFetchResult, MacroReferenceSource,
        MacroReferenceSourceError,
    },
};
use reqwest::Url;

use crate::{SOURCE_NAME, official_http::OfficialHttpClient};

const FILES_URL: &str = "https://www.e-stat.go.jp/stat-search/files";
const MAX_CATALOG_PAGES: usize = 60;
const PERIOD_SEARCH_OFFSETS: [i32; 4] = [-2, -1, 0, 1];

pub struct EStatMacroReferenceSource {
    http: OfficialHttpClient,
}

impl EStatMacroReferenceSource {
    pub fn new(redis_url: &str) -> Result<Self, String> {
        let http = OfficialHttpClient::new(redis_url).map_err(|error| error.to_string())?;
        Ok(Self { http })
    }

    async fn fetch_for_statistics_code(
        &self,
        stat_code: &str,
        targets: Vec<MacroReferenceTarget>,
    ) -> Vec<MacroReferenceFetchResult> {
        let listing_url = statistics_listing_url(stat_code);
        let Some(summary_kind) = monthly_summary_kind(stat_code) else {
            return retry_all(targets, "e-Stat statistics code is unsupported");
        };
        let first_page_url = match catalog_url(stat_code, 1) {
            Ok(url) => url,
            Err(error) => {
                return targets
                    .into_iter()
                    .map(|target| MacroReferenceFetchResult::retry(target, error.clone()))
                    .collect();
            }
        };
        let catalog_html = match self.http.get_text_cached(&first_page_url).await {
            Ok(html) => html,
            Err(error) => {
                return targets
                    .into_iter()
                    .map(|target| MacroReferenceFetchResult::retry(target, error.to_string()))
                    .collect();
            }
        };
        let Some(first_page) =
            parser::parse_catalog_page(&catalog_html, &first_page_url, summary_kind)
        else {
            return retry_all(
                targets,
                "e-Stat statistics file catalog could not be parsed",
            );
        };
        if first_page.page_count > MAX_CATALOG_PAGES {
            return retry_all(
                targets,
                "e-Stat statistics file catalog exceeds lookup page budget",
            );
        }

        let mut groups = first_page.groups;
        for page in 2..=first_page.page_count {
            let page_url = match catalog_url(stat_code, page) {
                Ok(url) => url,
                Err(error) => return retry_all(targets, error),
            };
            let html = match self.http.get_text_cached(&page_url).await {
                Ok(html) => html,
                Err(error) => return retry_all(targets, error.to_string()),
            };
            let Some(catalog_page) = parser::parse_catalog_page(&html, &page_url, summary_kind)
            else {
                return retry_all(
                    targets,
                    "e-Stat statistics file catalog could not be parsed",
                );
            };
            groups.extend(catalog_page.groups);
        }

        let target_periods = targets
            .iter()
            .filter_map(|target| target_publication_datetime(target).map(|date| (target, date)))
            .map(|(target, date)| {
                (
                    target_key(target),
                    PERIOD_SEARCH_OFFSETS
                        .into_iter()
                        .filter_map(|offset| month_offset(date.date(), offset))
                        .collect::<HashSet<_>>(),
                )
            })
            .collect::<HashMap<_, _>>();
        let mut relevant_groups = HashMap::<String, parser::MonthlyGroup>::new();
        for group in groups {
            if target_periods
                .values()
                .any(|periods| periods.contains(&group.period))
            {
                relevant_groups.entry(group.url.clone()).or_insert(group);
            }
        }

        let mut records_by_group = Vec::new();
        let mut failed_group_periods = HashSet::new();
        for group in relevant_groups.into_values() {
            let html = match self.http.get_text_cached(&group.url).await {
                Ok(html) => html,
                Err(_) => {
                    failed_group_periods.insert(group.period);
                    continue;
                }
            };
            match parser::parse_monthly_documents(&html, &group.url) {
                Some(records) => records_by_group.push((group, records)),
                None => {
                    failed_group_periods.insert(group.period);
                }
            }
        }

        let mut references_by_target =
            HashMap::<(String, String), Vec<MacroOfficialReference>>::new();
        let mut failed_targets = HashSet::new();
        for target in &targets {
            let key = target_key(target);
            let Some(published_at) = target_publication_datetime(target) else {
                failed_targets.insert(key);
                continue;
            };
            let candidate_periods = target_periods.get(&key).cloned().unwrap_or_default();
            for (group, records) in &records_by_group {
                if !candidate_periods.contains(&group.period) {
                    continue;
                }
                for record in records
                    .iter()
                    .filter(|record| record.publication_date == published_at.date())
                {
                    let detail_html = match self.http.get_text_cached(&record.detail_url).await {
                        Ok(html) => html,
                        Err(_) => {
                            failed_targets.insert(key.clone());
                            continue;
                        }
                    };
                    match parser::parse_file_detail(
                        &detail_html,
                        &record.detail_url,
                        stat_code,
                        &published_at,
                        target,
                    ) {
                        Ok(Some(reference)) => references_by_target
                            .entry(key.clone())
                            .or_default()
                            .push(reference),
                        Ok(None) => {}
                        Err(_) => {
                            failed_targets.insert(key.clone());
                        }
                    }
                }
            }
        }

        targets
            .into_iter()
            .map(|target| {
                let key = target_key(&target);
                let group_failed = target_periods.get(&key).is_some_and(|periods| {
                    periods
                        .iter()
                        .any(|period| failed_group_periods.contains(period))
                });
                if failed_targets.contains(&key) || group_failed {
                    return MacroReferenceFetchResult::retry(
                        target,
                        "e-Stat file metadata lookup failed",
                    );
                }
                match references_by_target.remove(&key).unwrap_or_default() {
                    references if !references.is_empty() => {
                        MacroReferenceFetchResult::success(target, references)
                    }
                    _ => MacroReferenceFetchResult::success(
                        target.clone(),
                        vec![parser::listing_reference(&target, &listing_url)],
                    ),
                }
            })
            .collect()
    }
}

#[async_trait]
impl MacroReferenceSource for EStatMacroReferenceSource {
    fn supports(&self, target: &MacroReferenceTarget) -> bool {
        target.event_source == SOURCE_NAME
            && target
                .event_external_id
                .split_once(':')
                .is_some_and(|(stat_code, _)| is_supported_statistics_code(stat_code))
    }

    async fn fetch_references(
        &self,
        targets: &[MacroReferenceTarget],
    ) -> Result<Vec<MacroReferenceFetchResult>, MacroReferenceSourceError> {
        let mut grouped = HashMap::<String, Vec<MacroReferenceTarget>>::new();
        for target in targets.iter().filter(|target| self.supports(target)) {
            if let Some((stat_code, _)) = target.event_external_id.split_once(':') {
                grouped
                    .entry(stat_code.to_owned())
                    .or_default()
                    .push(target.clone());
            }
        }

        let mut results = Vec::new();
        for (stat_code, code_targets) in grouped {
            results.extend(
                self.fetch_for_statistics_code(&stat_code, code_targets)
                    .await,
            );
        }
        Ok(results)
    }
}

fn is_supported_statistics_code(stat_code: &str) -> bool {
    matches!(stat_code, "00200573" | "00200531")
}

fn monthly_summary_kind(stat_code: &str) -> Option<parser::MonthlySummaryKind> {
    match stat_code {
        "00200573" => Some(parser::MonthlySummaryKind::ConsumerPrices),
        "00200531" => Some(parser::MonthlySummaryKind::LaborForce),
        _ => None,
    }
}

fn catalog_url(stat_code: &str, page: usize) -> Result<String, String> {
    let mut url = Url::parse(FILES_URL).map_err(|error| error.to_string())?;
    url.query_pairs_mut()
        .append_pair("layout", "datasetList")
        .append_pair("toukei", stat_code)
        .append_pair("page", &page.to_string());
    Ok(url.to_string())
}

fn statistics_listing_url(stat_code: &str) -> String {
    format!("{FILES_URL}?layout=datasetList&toukei={stat_code}")
}

fn target_publication_datetime(target: &MacroReferenceTarget) -> Option<NaiveDateTime> {
    let (stat_code, datetime) = target.event_external_id.split_once(':')?;
    (target.event_source == SOURCE_NAME && is_supported_statistics_code(stat_code))
        .then(|| NaiveDateTime::parse_from_str(datetime, "%Y%m%d%H%M").ok())
        .flatten()
}

fn target_key(target: &MacroReferenceTarget) -> (String, String) {
    (
        target.event_source.clone(),
        target.event_external_id.clone(),
    )
}

fn month_offset(date: NaiveDate, offset: i32) -> Option<(i32, u32)> {
    let month_index = date
        .year()
        .checked_mul(12)?
        .checked_add(date.month() as i32 - 1)?;
    let target_index = month_index.checked_add(offset)?;
    let year = target_index.div_euclid(12);
    let month = u32::try_from(target_index.rem_euclid(12) + 1).ok()?;
    NaiveDate::from_ymd_opt(year, month, 1).map(|_| (year, month))
}

fn retry_all(
    targets: Vec<MacroReferenceTarget>,
    failure: impl Into<String>,
) -> Vec<MacroReferenceFetchResult> {
    let failure = failure.into();
    targets
        .into_iter()
        .map(|target| MacroReferenceFetchResult::retry(target, failure.clone()))
        .collect()
}
