use std::collections::HashMap;

use chrono::{NaiveDate, NaiveDateTime};
use core_application::{
    calendar::macro_reference_targets::MacroReferenceTarget,
    macro_reference_source::MacroOfficialReference,
};
use core_domain::calendar_event::CalendarEventReferenceKind;
use reqwest::Url;
use scraper::{ElementRef, Html, Selector};

use crate::SOURCE_NAME;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct MonthlyGroup {
    pub period: (i32, u32),
    pub url: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct MonthlyDocument {
    pub publication_date: NaiveDate,
    pub title: String,
    pub detail_url: String,
}

pub(super) struct CatalogPage {
    pub page_count: usize,
    pub groups: Vec<MonthlyGroup>,
}

#[derive(Clone, Copy)]
pub(super) enum MonthlySummaryKind {
    ConsumerPrices,
    LaborForce,
}

pub(super) fn parse_catalog_page(
    html: &str,
    page_url: &str,
    summary_kind: MonthlySummaryKind,
) -> Option<CatalogPage> {
    let document = Html::parse_document(html);
    let body_selector = Selector::parse(".stat-resource_list-body").ok()?;
    document.select(&body_selector).next()?;
    let page_count = document
        .select(&Selector::parse(".stat-paginate-index").ok()?)
        .next()
        .and_then(|element| parse_page_count(&element_text(&element)))
        .unwrap_or(1);
    let article_selector = Selector::parse("article.stat-resource_list-item-dataset").ok()?;
    let link_selector = Selector::parse("a[href]").ok()?;
    let base = Url::parse(page_url).ok()?;
    let groups = document
        .select(&article_selector)
        .filter_map(|article| {
            let text = element_text(&article);
            let period = parse_month_after_label(&text, "調査年月")?;
            let link = article.select(&link_selector).find(|link| {
                let title = element_text(link);
                is_monthly_summary_group(summary_kind, &title)
            })?;
            let url = resolve_official_url(&base, link.value().attr("href")?)?;
            let url = monthly_list_url(&url)?;
            Some(MonthlyGroup { period, url })
        })
        .collect();
    Some(CatalogPage { page_count, groups })
}

pub(super) fn parse_monthly_documents(html: &str, page_url: &str) -> Option<Vec<MonthlyDocument>> {
    let document = Html::parse_document(html);
    let body_selector = Selector::parse(".stat-resource_list-body").ok()?;
    document.select(&body_selector).next()?;
    let article_selector = Selector::parse("article.stat-dataset_list-item").ok()?;
    let link_selector = Selector::parse("a[href*='stat_infid=']").ok()?;
    let base = Url::parse(page_url).ok()?;
    Some(
        document
            .select(&article_selector)
            .filter_map(|article| {
                let link = article
                    .select(&link_selector)
                    .find(|link| element_text(link).contains("結果の概要"))?;
                let text = element_text(&article);
                let publication_date = parse_date_after_label(&text, "公開（更新）日")?;
                let detail_url = resolve_official_url(&base, link.value().attr("href")?)?;
                Some(MonthlyDocument {
                    publication_date,
                    title: element_text(&link),
                    detail_url,
                })
            })
            .collect(),
    )
}

pub(super) fn parse_file_detail(
    html: &str,
    detail_url: &str,
    expected_stat_code: &str,
    expected_publication_datetime: &NaiveDateTime,
    target: &MacroReferenceTarget,
) -> Result<Option<MacroOfficialReference>, String> {
    let document = Html::parse_document(html);
    let row_selector =
        Selector::parse("table.stat-dataset_sheet tr").map_err(|error| error.to_string())?;
    let cell_selector = Selector::parse("th, td").map_err(|error| error.to_string())?;
    let mut fields = HashMap::<String, String>::new();
    for row in document.select(&row_selector) {
        let cells = row.select(&cell_selector).collect::<Vec<_>>();
        if cells.len() < 2 {
            continue;
        }
        fields.insert(element_text(&cells[0]), element_text(&cells[1]));
    }
    let actual_stat_code = required_field(&fields, "政府統計コード")?;
    let publication_value = required_field(&fields, "公開年月日時分")?;
    let period_value = required_field(&fields, "調査年月")?;
    let title = required_field(&fields, "統計表名")?;
    let actual_publication_datetime =
        NaiveDateTime::parse_from_str(publication_value, "%Y-%m-%d %H:%M")
            .map_err(|error| format!("e-Stat publication datetime is invalid: {error}"))?;
    let (year, month) = parse_japanese_year_month(period_value)
        .ok_or_else(|| "e-Stat survey period is invalid".to_owned())?;
    let period_start = NaiveDate::from_ymd_opt(year, month, 1)
        .ok_or_else(|| "e-Stat survey period is out of range".to_owned())?;
    let period_end = last_day_of_month(year, month)
        .ok_or_else(|| "e-Stat survey period is out of range".to_owned())?;
    if actual_stat_code != expected_stat_code
        || actual_publication_datetime != *expected_publication_datetime
    {
        return Ok(None);
    }

    let base = Url::parse(detail_url).map_err(|error| error.to_string())?;
    let link_selector = Selector::parse("a[href]").map_err(|error| error.to_string())?;
    let pdf_url = document
        .select(&link_selector)
        .find(|link| element_text(link).eq_ignore_ascii_case("PDF"))
        .and_then(|link| link.value().attr("href"))
        .and_then(|href| resolve_official_url(&base, href));
    let public_url = pdf_url.unwrap_or_else(|| detail_url.to_owned());
    let publication_time = actual_publication_datetime.format("%Y-%m-%d %H:%M");

    Ok(Some(MacroOfficialReference {
        target: target.clone(),
        kind: CalendarEventReferenceKind::IndividualDocument,
        source: SOURCE_NAME.to_owned(),
        external_id: detail_url.to_owned(),
        title: title.to_owned(),
        url: Some(public_url),
        target_period_start: Some(period_start),
        target_period_end: Some(period_end),
        target_period_type: Some("month".to_owned()),
        publication_evidence: Some(format!(
            "e-Stat file metadata lists publication datetime {publication_time} for survey month {period_value}"
        )),
        publication_evidence_url: Some(detail_url.to_owned()),
    }))
}

pub(super) fn listing_reference(
    target: &MacroReferenceTarget,
    listing_url: &str,
) -> MacroOfficialReference {
    MacroOfficialReference {
        target: target.clone(),
        kind: CalendarEventReferenceKind::OfficialListing,
        source: SOURCE_NAME.to_owned(),
        external_id: format!("{}:official-listing", target.event_external_id),
        title: "公式統計資料一覧".to_owned(),
        url: Some(listing_url.to_owned()),
        target_period_start: None,
        target_period_end: None,
        target_period_type: None,
        publication_evidence: None,
        publication_evidence_url: None,
    }
}

fn is_monthly_summary_group(summary_kind: MonthlySummaryKind, title: &str) -> bool {
    match summary_kind {
        MonthlySummaryKind::ConsumerPrices => title.contains("月報") && title.contains("月次"),
        MonthlySummaryKind::LaborForce => {
            title.contains("結果の概要") && title.contains("基本集計") && title.contains("月次")
        }
    }
}

fn parse_page_count(value: &str) -> Option<usize> {
    let (_, total) = value.split_once('/')?;
    total
        .trim()
        .trim_end_matches("ページ")
        .parse::<usize>()
        .ok()
}

fn required_field<'a>(fields: &'a HashMap<String, String>, name: &str) -> Result<&'a str, String> {
    fields
        .get(name)
        .map(String::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("e-Stat file detail is missing {name}"))
}

fn parse_month_after_label(value: &str, label: &str) -> Option<(i32, u32)> {
    let (_, period) = value.split_once(label)?;
    parse_japanese_year_month(period)
}

fn parse_date_after_label(value: &str, label: &str) -> Option<NaiveDate> {
    let (_, published_at) = value.split_once(label)?;
    NaiveDate::parse_from_str(published_at.split_whitespace().next()?, "%Y-%m-%d").ok()
}

fn parse_japanese_year_month(value: &str) -> Option<(i32, u32)> {
    let normalized = value
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect::<String>();
    let year_position = normalized.find('年')?;
    let year = normalized[..year_position].parse::<i32>().ok()?;
    let after_year = &normalized[year_position + '年'.len_utf8()..];
    let month_end = after_year.find('月')?;
    let month = after_year[..month_end].parse::<u32>().ok()?;
    NaiveDate::from_ymd_opt(year, month, 1).map(|_| (year, month))
}

fn resolve_official_url(base: &Url, href: &str) -> Option<String> {
    let url = base.join(href).ok()?;
    if url.origin() != base.origin() {
        return None;
    }
    matches!(url.scheme(), "http" | "https").then(|| url.to_string())
}

fn monthly_list_url(value: &str) -> Option<String> {
    let mut url = Url::parse(value).ok()?;
    let pairs = url
        .query_pairs()
        .filter(|(key, _)| key != "layout")
        .map(|(key, value)| (key.into_owned(), value.into_owned()))
        .collect::<Vec<_>>();
    url.set_query(None);
    url.query_pairs_mut()
        .extend_pairs(
            pairs
                .iter()
                .map(|(key, value)| (key.as_str(), value.as_str())),
        )
        .append_pair("layout", "datalist");
    Some(url.to_string())
}

fn element_text(element: &ElementRef<'_>) -> String {
    element
        .text()
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn last_day_of_month(year: i32, month: u32) -> Option<NaiveDate> {
    let (next_year, next_month) = if month == 12 {
        (year.checked_add(1)?, 1)
    } else {
        (year, month + 1)
    };
    NaiveDate::from_ymd_opt(next_year, next_month, 1)?.pred_opt()
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;
    use core_application::{
        calendar::macro_reference_targets::MacroReferenceTarget,
        macro_reference_source::MacroOfficialReference,
    };
    use core_domain::calendar_event::CalendarEventReferenceKind;
    use indoc::indoc;

    use super::{
        MonthlyDocument, MonthlyGroup, MonthlySummaryKind, parse_catalog_page, parse_file_detail,
        parse_monthly_documents,
    };

    fn target(external_id: &str) -> MacroReferenceTarget {
        MacroReferenceTarget {
            event_source: "e_stat".to_owned(),
            event_external_id: external_id.to_owned(),
            event_title: "架空の統計イベント".to_owned(),
            same_day_event_count: 1,
        }
    }

    #[test]
    fn catalog_page_selects_monthly_result_groups_with_dynamic_query_filters() {
        let html = indoc! {
            r#"
            <div class="stat-paginate-index">1/1ページ</div>
            <div class="stat-resource_list-body">
              <article class="stat-resource_list-item-dataset">
                <ul class="stat-resource_list-detail">
                  <li>架空の統計</li>
                  <li><a href="/stat-search/files?page=1&amp;layout=datasetList&amp;toukei=sample-stat&amp;tstat=sample-series&amp;year=20990&amp;month=20990401&amp;tclass1=sample-monthly">架空の統計 月報 / 月次</a></li>
                  <li><span>調査年月</span> 2099年 4月</li>
                </ul>
              </article>
              <article class="stat-resource_list-item-dataset">
                <ul><li><a href="/stat-search/files?layout=datasetList&amp;toukei=sample-stat">架空の統計 長期時系列</a></li><li>調査年月 2099年4月</li></ul>
              </article>
            </div>
            "#
        };

        let actual = parse_catalog_page(
            html,
            "https://example.invalid/stat-search/files?layout=datasetList&toukei=sample-stat&page=1",
            MonthlySummaryKind::ConsumerPrices,
        );

        assert_eq!(
            actual.map(|page| (page.page_count, page.groups)),
            Some((
                1,
                vec![MonthlyGroup {
                    period: (2099, 4),
                    url: "https://example.invalid/stat-search/files?page=1&toukei=sample-stat&tstat=sample-series&year=20990&month=20990401&tclass1=sample-monthly&layout=datalist".to_owned(),
                }],
            )),
        );
    }

    #[test]
    fn monthly_document_list_keeps_all_same_day_summaries() {
        let html = indoc! {
            r#"
            <div class="stat-resource_list-body">
              <article class="stat-dataset_list-item">
                <a href="/stat-search/files?stat_infid=sample-a">結果の概要（架空の地域 A）</a>
                <div><span class="stat-sp">調査年月</span>2099年4月</div>
                <div><span class="stat-sp">公開（更新）日</span>2099-05-17</div>
              </article>
              <article class="stat-dataset_list-item">
                <a href="/stat-search/files?stat_infid=sample-b">結果の概要（架空の地域 B）</a>
                <div><span class="stat-sp">公開（更新）日</span>2099-05-17</div>
              </article>
            </div>
            "#
        };
        let page_url =
            "https://example.invalid/stat-search/files?layout=datalist&toukei=sample-stat";

        let actual = parse_monthly_documents(html, page_url);

        assert_eq!(
            actual,
            Some(vec![
                MonthlyDocument {
                    publication_date: NaiveDate::from_ymd_opt(2099, 5, 17)
                        .unwrap_or(NaiveDate::MIN),
                    title: "結果の概要（架空の地域 A）".to_owned(),
                    detail_url: "https://example.invalid/stat-search/files?stat_infid=sample-a"
                        .to_owned(),
                },
                MonthlyDocument {
                    publication_date: NaiveDate::from_ymd_opt(2099, 5, 17)
                        .unwrap_or(NaiveDate::MIN),
                    title: "結果の概要（架空の地域 B）".to_owned(),
                    detail_url: "https://example.invalid/stat-search/files?stat_infid=sample-b"
                        .to_owned(),
                },
            ]),
        );
    }

    #[test]
    fn file_detail_requires_exact_code_and_publication_datetime() {
        let html = indoc! {
            r#"
            <table class="stat-dataset_sheet"><tbody>
              <tr><th>政府統計コード</th><td>sample-stat</td></tr>
              <tr><th>統計表名</th><td>結果の概要（架空の地域 A）</td></tr>
              <tr><th>調査年月</th><td>2099年 4月</td></tr>
              <tr><th>公開年月日時分</th><td>2099-05-17 08:30</td></tr>
            </tbody></table>
            <a href="/files/sample-summary.pdf">PDF</a>
            "#
        };
        let detail_url = "https://example.invalid/stat-search/files?stat_infid=sample-a";
        let target = target("sample-stat:209905170830");

        let actual = parse_file_detail(
            html,
            detail_url,
            "sample-stat",
            &NaiveDate::from_ymd_opt(2099, 5, 17)
                .and_then(|date| date.and_hms_opt(8, 30, 0))
                .unwrap_or(NaiveDate::MIN.and_hms_opt(0, 0, 0).unwrap_or_default()),
            &target,
        );

        assert_eq!(
            actual,
            Ok(Some(MacroOfficialReference {
                target,
                kind: CalendarEventReferenceKind::IndividualDocument,
                source: "e_stat".to_owned(),
                external_id: detail_url.to_owned(),
                title: "結果の概要（架空の地域 A）".to_owned(),
                url: Some("https://example.invalid/files/sample-summary.pdf".to_owned()),
                target_period_start: NaiveDate::from_ymd_opt(2099, 4, 1),
                target_period_end: NaiveDate::from_ymd_opt(2099, 4, 30),
                target_period_type: Some("month".to_owned()),
                publication_evidence: Some("e-Stat file metadata lists publication datetime 2099-05-17 08:30 for survey month 2099年 4月".to_owned()),
                publication_evidence_url: Some(detail_url.to_owned()),
            })),
        );
    }

    #[test]
    fn file_detail_rejects_a_different_publication_datetime() {
        let html = indoc! {
            r#"
            <table class="stat-dataset_sheet"><tbody>
              <tr><th>政府統計コード</th><td>sample-stat</td></tr>
              <tr><th>統計表名</th><td>結果の概要（架空の地域 A）</td></tr>
              <tr><th>調査年月</th><td>2099年 4月</td></tr>
              <tr><th>公開年月日時分</th><td>2099-05-17 08:30</td></tr>
            </tbody></table>
            "#
        };
        let target = target("sample-stat:209905180830");

        let actual = parse_file_detail(
            html,
            "https://example.invalid/stat-search/files?stat_infid=sample-a",
            "sample-stat",
            &NaiveDate::from_ymd_opt(2099, 5, 18)
                .and_then(|date| date.and_hms_opt(8, 30, 0))
                .unwrap_or(NaiveDate::MIN.and_hms_opt(0, 0, 0).unwrap_or_default()),
            &target,
        );

        assert_eq!(actual, Ok(None));
    }
}
