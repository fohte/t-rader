mod common;
mod iip;

pub(super) use iip::{parse_iip_archive, parse_iip_publication_date, parse_iip_release_page};

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;
    use core_application::{
        calendar::macro_reference_targets::MacroReferenceTarget,
        macro_reference_source::MacroOfficialReference,
    };
    use core_domain::calendar_event::CalendarEventReferenceKind;
    use indoc::indoc;

    use super::iip::IipReleasePage;
    use super::*;

    fn target(published_at: &str) -> MacroReferenceTarget {
        MacroReferenceTarget {
            event_source: "e_stat".to_owned(),
            event_external_id: format!("sample-stat-code:{published_at}"),
            event_title: "架空の統計イベント".to_owned(),
            same_day_event_count: 1,
        }
    }

    #[test]
    fn archive_includes_each_period_and_release_status() {
        let archive = indoc! {
            r#"
            <table class="press-table"><tbody>
              <tr><td><a href="result/sample_209910sj.html">結果の概要</a></td></tr>
              <tr><td><a href="result/sample_209910kj.html">結果の概要</a></td></tr>
              <tr><td><a href="result/unknown.html">結果の概要</a></td></tr>
            </tbody></table>
            "#
        };
        let archive_url = "https://example.invalid/industrial/archive.html";

        assert_eq!(
            parse_iip_archive(archive, archive_url),
            Some(vec![
                IipReleasePage {
                    title: "結果の概要".to_owned(),
                    url: "https://example.invalid/industrial/result/sample_209910sj.html"
                        .to_owned(),
                    period_start: NaiveDate::from_ymd_opt(2099, 10, 1).unwrap_or(NaiveDate::MIN),
                },
                IipReleasePage {
                    title: "結果の概要".to_owned(),
                    url: "https://example.invalid/industrial/result/sample_209910kj.html"
                        .to_owned(),
                    period_start: NaiveDate::from_ymd_opt(2099, 10, 1).unwrap_or(NaiveDate::MIN),
                },
            ]),
        );
    }

    #[test]
    fn release_page_requires_exact_publication_date_and_explicit_period() {
        let page = indoc! {
            r#"
            <html><head>
              <meta property="og:description" content="架空の生産指数速報（2099年10月分）">
            </head><body>
              <h1>架空の生産・出荷・在庫指数速報</h1>
              <p>結果の概要（2099年11月15日発表）</p>
            </body></html>
            "#
        };
        let page_url = "https://example.invalid/industrial/result/sample_209910sj.html";

        assert_eq!(
            parse_iip_release_page(page, page_url, &target("209911150850"), "sample-stat-code"),
            Some(MacroOfficialReference {
                target: target("209911150850"),
                kind: CalendarEventReferenceKind::IndividualDocument,
                source: "meti_jp".to_owned(),
                external_id: page_url.to_owned(),
                title: "架空の生産・出荷・在庫指数速報".to_owned(),
                url: Some(page_url.to_owned()),
                target_period_start: NaiveDate::from_ymd_opt(2099, 10, 1),
                target_period_end: NaiveDate::from_ymd_opt(2099, 10, 31),
                target_period_type: Some("month".to_owned()),
                publication_evidence: Some(
                    "Official industrial production result page states publication date 2099-11-15 for 2099年10月".to_owned(),
                ),
                publication_evidence_url: Some(page_url.to_owned()),
            }),
        );
    }

    #[test]
    fn release_page_rejects_a_different_candidate_date() {
        let page = indoc! {
            r#"
            <html><head>
              <meta property="og:description" content="架空の生産指数速報（2099年10月分）">
            </head><body>
              <h1>架空の生産・出荷・在庫指数速報</h1>
              <p>結果の概要（2099年11月15日発表）</p>
            </body></html>
            "#
        };

        assert_eq!(
            parse_iip_release_page(
                page,
                "https://example.invalid/industrial/result/sample_209910sj.html",
                &target("209911160850"),
                "sample-stat-code",
            ),
            None,
        );
    }
}
