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

mod official_http;
mod parser;

const CUSTOMS_ARCHIVE_URL: &str = "https://www.customs.go.jp/toukei/shinbun/happyou.htm";
const E_STAT_TRADE_CODE: &str = "00350300";

pub struct CustomsMacroReferenceSource {
    http: OfficialHttpClient,
}

impl CustomsMacroReferenceSource {
    pub fn new(redis_url: &str) -> Result<Self, String> {
        let http = OfficialHttpClient::new(redis_url).map_err(|error| error.to_string())?;
        Ok(Self { http })
    }
}

#[async_trait]
impl MacroReferenceSource for CustomsMacroReferenceSource {
    fn supports(&self, target: &MacroReferenceTarget) -> bool {
        target.event_source == "e_stat"
            && target
                .event_external_id
                .split_once(':')
                .is_some_and(|(code, _)| code == E_STAT_TRADE_CODE)
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

        let html = match self.http.get_text_cached(CUSTOMS_ARCHIVE_URL).await {
            Ok(html) => html,
            Err(error) => {
                let failure = error.to_string();
                return Ok(supported_targets
                    .into_iter()
                    .map(|target| MacroReferenceFetchResult::retry(target, failure.clone()))
                    .collect());
            }
        };
        let Some(rows) = parser::parse_customs_rows(&html, CUSTOMS_ARCHIVE_URL) else {
            return Ok(supported_targets
                .into_iter()
                .map(|target| {
                    MacroReferenceFetchResult::retry(
                        target,
                        "customs archive response could not be parsed",
                    )
                })
                .collect());
        };

        Ok(supported_targets
            .into_iter()
            .map(|target| {
                let publication_date = parse_external_publication_date(&target.event_external_id);
                let matching_rows = publication_date
                    .map(|date| {
                        rows.iter()
                            .filter(|row| row.publication_date == Some(date))
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                let has_unidentified_rows = rows.iter().any(|row| row.publication_date.is_none());
                result_for_matches(
                    &target,
                    &matching_rows,
                    has_unidentified_rows,
                    CUSTOMS_ARCHIVE_URL,
                )
            })
            .collect())
    }
}

fn result_for_matches(
    target: &MacroReferenceTarget,
    matching_rows: &[&parser::JapaneseReleaseRow],
    has_unidentified_rows: bool,
    archive_url: &str,
) -> MacroReferenceFetchResult {
    if target.same_day_event_count != 1 || matching_rows.len() != 1 || has_unidentified_rows {
        return listing_result(target);
    }
    let Some(document) = matching_rows
        .first()
        .and_then(|row| parser::customs_document(target, row, archive_url))
    else {
        return listing_result(target);
    };
    MacroReferenceFetchResult::success(target.clone(), vec![document])
}

fn parse_external_publication_date(external_id: &str) -> Option<NaiveDate> {
    let (stat_code, published_at) = external_id.split_once(':')?;
    if stat_code != E_STAT_TRADE_CODE || published_at.len() != 12 {
        return None;
    }
    chrono::NaiveDateTime::parse_from_str(published_at, "%Y%m%d%H%M")
        .ok()
        .map(|date_time| date_time.date())
}

fn listing_result(target: &MacroReferenceTarget) -> MacroReferenceFetchResult {
    MacroReferenceFetchResult::success(
        target.clone(),
        vec![MacroOfficialReference {
            target: target.clone(),
            kind: CalendarEventReferenceKind::OfficialListing,
            source: "customs_jp".to_owned(),
            external_id: format!("trade-release-listing:{}", target.event_external_id),
            title: "公式統計資料一覧".to_owned(),
            url: Some(CUSTOMS_ARCHIVE_URL.to_owned()),
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
        macro_reference_source::MacroReferenceFetchResult,
    };
    use indoc::indoc;

    use super::{
        listing_result,
        parser::{customs_document, parse_customs_rows},
        result_for_matches,
    };

    #[test]
    fn individual_link_requires_unique_calendar_and_official_records() {
        let archive = indoc! {
            r#"
            <table>
              <tbody>
                <tr><td>2099年3月</td><td>2099年4月8日（速報）</td><td><a href="/archive/sample-a.pdf">PDF</a></td></tr>
                <tr><td>2099年2月</td><td>2099年4月8日（確報）</td><td><a href="/archive/sample-b.pdf">PDF</a></td></tr>
              </tbody>
            </table>
            "#
        };
        let target = MacroReferenceTarget {
            event_source: "e_stat".to_owned(),
            event_external_id: "sample-stat-code:209904080850".to_owned(),
            event_title: "架空の統計イベント".to_owned(),
            same_day_event_count: 1,
        };
        let archive_url = "https://example.invalid/archive";
        let publication_date = NaiveDate::from_ymd_opt(2099, 4, 8).unwrap_or(NaiveDate::MIN);
        let rows = parse_customs_rows(archive, archive_url).unwrap_or_default();
        let matching_rows = rows
            .iter()
            .filter(|row| row.publication_date == Some(publication_date))
            .collect::<Vec<_>>();
        let unique_target_rows = &matching_rows[..1];
        let duplicate_calendar_target = MacroReferenceTarget {
            same_day_event_count: 2,
            ..target.clone()
        };
        let unidentified_archive = indoc! {
            r#"
            <table><tbody>
              <tr><td>2099年1月</td><td>公開日未確認</td><td><a href="/archive/sample-c.pdf">PDF</a></td></tr>
            </tbody></table>
            "#
        };
        let has_unidentified_rows = parse_customs_rows(unidentified_archive, archive_url)
            .is_some_and(|rows| rows.iter().any(|row| row.publication_date.is_none()));
        let unique_document = unique_target_rows
            .first()
            .and_then(|row| customs_document(&target, row, archive_url));
        let actual = (
            result_for_matches(&target, &matching_rows, false, archive_url),
            result_for_matches(&target, unique_target_rows, false, archive_url),
            result_for_matches(
                &duplicate_calendar_target,
                unique_target_rows,
                false,
                archive_url,
            ),
            result_for_matches(
                &target,
                unique_target_rows,
                has_unidentified_rows,
                archive_url,
            ),
        );

        assert_eq!(
            actual,
            (
                listing_result(&target),
                MacroReferenceFetchResult::success(
                    target.clone(),
                    unique_document.into_iter().collect(),
                ),
                listing_result(&duplicate_calendar_target),
                listing_result(&target),
            ),
        );
    }
}
