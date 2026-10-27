use std::collections::HashMap;
use std::ops::Range;

use serde::Serialize;

use crate::markdown_lint::lint_id;
use crate::markdown_lint::market_numeric::{MarketNumericFinding, scan_market_numeric_source};
pub use crate::markdown_lint::warning_id_group;
use crate::note_graph::GraphDef;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct NoteBodyWarning {
    pub id: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoteBodyWarningFinding {
    pub warning: NoteBodyWarning,
    pub value: String,
    pub path: Option<String>,
    pub range: Range<usize>,
}

pub fn scan_note_warnings_with_ids(body: &str, graphs: &[GraphDef]) -> Vec<NoteBodyWarning> {
    scan_note_warning_findings_with_ids(body, graphs)
        .into_iter()
        .map(|finding| finding.warning)
        .collect()
}

pub fn scan_note_warning_findings_with_ids(
    body: &str,
    graphs: &[GraphDef],
) -> Vec<NoteBodyWarningFinding> {
    let mut occurrences = HashMap::<(String, String), usize>::new();
    let mut findings = scan_text_with_ids(body, None, None, &mut occurrences);
    for (graph_index, graph) in graphs.iter().enumerate() {
        for (node_index, node) in graph.nodes.iter().enumerate() {
            let message_location = format!(
                "graphs[{graph_index}] (id={:?}).nodes[{node_index}] (id={:?}).label",
                graph.id, node.id
            );
            let path = format!("graphs[{graph_index}].nodes[{node_index}].label");
            findings.extend(scan_text_with_ids(
                &node.label,
                Some(path),
                Some(&message_location),
                &mut occurrences,
            ));
        }
        for (edge_index, edge) in graph.edges.iter().enumerate() {
            let Some(label) = edge.label.as_deref() else {
                continue;
            };
            let message_location = format!(
                "graphs[{graph_index}] (id={:?}).edges[{edge_index}] ({:?} -> {:?}).label",
                graph.id, edge.source, edge.target
            );
            let path = format!("graphs[{graph_index}].edges[{edge_index}].label");
            findings.extend(scan_text_with_ids(
                label,
                Some(path),
                Some(&message_location),
                &mut occurrences,
            ));
        }
    }
    findings
}

pub fn scan_graph_warnings_with_ids(
    graphs: &[GraphDef],
    preceding_warnings: &[NoteBodyWarning],
) -> Vec<NoteBodyWarning> {
    scan_graph_warning_findings_with_ids(graphs, preceding_warnings)
        .into_iter()
        .map(|finding| finding.warning)
        .collect()
}

pub fn scan_graph_warning_findings_with_ids(
    graphs: &[GraphDef],
    preceding_warnings: &[NoteBodyWarning],
) -> Vec<NoteBodyWarningFinding> {
    let mut occurrences = HashMap::<(String, String), usize>::new();
    for warning in preceding_warnings {
        let Some(group_id) = warning_id_group(&warning.id) else {
            continue;
        };
        let Some((rule_id, key)) = split_warning_group(group_id) else {
            continue;
        };
        let count = warning
            .id
            .rsplit_once(':')
            .and_then(|(_, count)| count.parse::<usize>().ok())
            .unwrap_or_default();
        occurrences
            .entry((rule_id.to_string(), key.to_string()))
            .and_modify(|current| *current = (*current).max(count))
            .or_insert(count);
    }
    let mut findings = Vec::new();
    for (graph_index, graph) in graphs.iter().enumerate() {
        for (node_index, node) in graph.nodes.iter().enumerate() {
            let message_location = format!(
                "graphs[{graph_index}] (id={:?}).nodes[{node_index}] (id={:?}).label",
                graph.id, node.id
            );
            let path = format!("graphs[{graph_index}].nodes[{node_index}].label");
            findings.extend(scan_text_with_ids(
                &node.label,
                Some(path),
                Some(&message_location),
                &mut occurrences,
            ));
        }
        for (edge_index, edge) in graph.edges.iter().enumerate() {
            let Some(label) = edge.label.as_deref() else {
                continue;
            };
            let message_location = format!(
                "graphs[{graph_index}] (id={:?}).edges[{edge_index}] ({:?} -> {:?}).label",
                graph.id, edge.source, edge.target
            );
            let path = format!("graphs[{graph_index}].edges[{edge_index}].label");
            findings.extend(scan_text_with_ids(
                label,
                Some(path),
                Some(&message_location),
                &mut occurrences,
            ));
        }
    }
    findings
}

fn split_warning_group(group_id: &str) -> Option<(&str, &str)> {
    let (rule_id, key) = group_id.split_once(':')?;
    (!rule_id.is_empty() && !key.is_empty()).then_some((rule_id, key))
}

fn scan_text_with_ids(
    text: &str,
    path: Option<String>,
    message_location: Option<&str>,
    occurrences: &mut HashMap<(String, String), usize>,
) -> Vec<NoteBodyWarningFinding> {
    scan_market_numeric_source(text)
        .into_iter()
        .map(|finding: MarketNumericFinding| {
            let occurrence = occurrences
                .entry((finding.kind.rule_id().to_string(), finding.key.clone()))
                .and_modify(|count| *count += 1)
                .or_insert(1);
            let id = lint_id(finding.kind.rule_id(), &finding.key, *occurrence);
            let message = message_location
                .map(|location| format!("{location}: {}", finding.message))
                .unwrap_or(finding.message);
            NoteBodyWarningFinding {
                warning: NoteBodyWarning { id, message },
                value: finding.key,
                path: path.clone(),
                range: finding.range,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use indoc::indoc;
    use rstest::rstest;

    use crate::note_graph::{GraphDef, GraphEdge, GraphNode, Layout};

    use super::{NoteBodyWarning, scan_graph_warnings_with_ids, scan_note_warnings_with_ids};

    fn price_warning(value: &str) -> String {
        format!(
            "価格候補の数値「{value}」がリンク外にあります。株価であれば、銘柄・日付・項目を確認して `[[price:<id>@<date>:<field>]]` で参照してください。株価ではない値ならそのまま残し、必要な理由を `warning_dispositions` に記載して警告を無視してください。"
        )
    }

    fn relative_warning(value: &str) -> String {
        format!(
            "相対表現「{value}」があります。株価の騰落率であれば、計算結果を手入力せず、対象期間の値を `[[change:<id>@<start>..<end>:<field>]]` で示してください。株価の騰落率ではない値はそのまま残し、必要な理由を `warning_dispositions` に記載して警告を無視してください。"
        )
    }

    fn bar_count_warning(value: &str) -> String {
        format!(
            "ローソク足の本数「{value}」があります。本数ではなく、開始日と終了日で対象期間を記載してください。"
        )
    }

    #[rstest]
    #[case::price_and_relative_values(
        "close 12,345円 then -6.0%",
        vec![
            price_warning("12,345円"),
            relative_warning("-6.0%"),
        ],
    )]
    #[case::small_price_with_spacing("for 950 円", vec![
        price_warning("950 円"),
    ])]
    #[case::bare_four_digit_price_candidate("9876", vec![
        price_warning("9876"),
    ])]
    #[case::scaled_non_price_counters_are_not_price_warnings(
        "1,200万件、3万人、4千回、5億台、6億社、7千個、2,400万円、5億ドル",
        vec![
            price_warning("2,400万円"),
            price_warning("5億ドル"),
        ],
    )]
    #[case::scaled_prices_before_counter_like_words_remain_warnings(
        "終値 1,200万株価、終値 2,400万日経、終値 5億本日、終値 3万回復",
        vec![
            price_warning("1,200万株"),
            price_warning("2,400万"),
            price_warning("5億"),
            price_warning("3万"),
        ],
    )]
    #[case::bar_counts(
        "73足、18 本、53本のローソク足、61営業日、34 営業日分、26取引日、45 取引日分、10637本",
        vec![
            bar_count_warning("73足"),
            bar_count_warning("18 本"),
            bar_count_warning("53本"),
            bar_count_warning("61営業日"),
            bar_count_warning("34 営業日分"),
            bar_count_warning("26取引日"),
            bar_count_warning("45 取引日分"),
            bar_count_warning("10637本"),
        ],
    )]
    #[case::price_before_negative_change("終値 987654 -4.2%", vec![
        price_warning("987654"),
        relative_warning("-4.2%"),
    ])]
    #[case::price_before_positive_change("close 86420 +3%", vec![
        price_warning("86420"),
        relative_warning("+3%"),
    ])]
    #[case::unmatched_closing_bracket_is_not_a_link("値上がりは5.0% ](内容)", vec![
        relative_warning("5.0%"),
    ])]
    #[case::relative_range("日中2〜3%が通常", vec![
        relative_warning("2〜3%"),
    ])]
    #[case::spaced_relative_range("日中 2 ~ 3% が通常", vec![
        relative_warning("2 ~ 3%"),
    ])]
    #[case::hyphen_relative_range("2-3%", vec![
        relative_warning("2-3%"),
    ])]
    #[case::ordinary_date_is_not_a_price("2030-01-02", vec![])]
    #[case::price_followed_by_honjitsu(
        "終値 2,673 本日は反落",
        vec![
        price_warning("2,673"),
        ],
    )]
    #[case::date_followed_by_ashimoto("9/6 足元では", vec![])]
    #[case::inline_link_url_is_not_scanned(
        "[123456円](https://news.example.com/articles/(2030)/12345678)",
        vec![
            price_warning("123456円"),
        ],
    )]
    #[case::relative_inline_link_url_is_not_scanned("[記事](/articles/2030/12345678)", vec![])]
    #[case::angle_delimited_relative_inline_link_url_is_not_scanned(
        "[記事](</articles/2030/12345678>)",
        vec![],
    )]
    #[case::autolink_url_is_not_scanned("<https://news.example.com/change/6.0%25>", vec![])]
    #[case::bare_url_is_not_scanned("https://example.com/12345678.", vec![])]
    #[case::bare_url_with_japanese_path_is_not_scanned(
        "https://news.example.com/73本/12345678.",
        vec![],
    )]
    #[case::japanese_punctuation_ends_bare_url(
        "https://example.com/a。次は5000円",
        vec![
            price_warning("5000円"),
        ],
    )]
    #[case::japanese_comma_ends_bare_url(
        "https://example.com/a、次は5000円",
        vec![
            price_warning("5000円"),
        ],
    )]
    #[case::japanese_text_ends_bare_url(
        "https://example.com/a次は5000円",
        vec![
            price_warning("5000円"),
        ],
    )]
    #[case::www_url_is_not_scanned("www.news.example.com/12345678", vec![])]
    #[case::links_are_not_scanned(
        "[[price:fictional-code@2030-01-02:close]] [[change:fictional-code@2030-01-02..2030-01-03:close]]",
        vec![],
    )]
    #[case::code_is_not_scanned(
        indoc! {"
            `12,345円 -6.0% 63足`

            ```text
            34,567円 +2.0% 63足
            ```
        "},
        vec![],
    )]
    fn scans_unlinked_note_body_warnings(#[case] body: &str, #[case] expected: Vec<String>) {
        assert_eq!(
            scan_note_warnings_with_ids(body, &[])
                .into_iter()
                .map(|warning| warning.message)
                .collect::<Vec<_>>(),
            expected,
        );
    }

    #[test]
    fn warning_ids_remain_stable_when_another_warning_is_added() {
        assert_eq!(
            (
                scan_note_warnings_with_ids("45,678円、45,678円", &[]),
                scan_note_warnings_with_ids("-3.5%、45,678円、45,678円", &[]),
            ),
            (
                vec![
                    NoteBodyWarning {
                        id: "price:45%2C678%E5%86%86:1".into(),
                        message: price_warning("45,678円"),
                    },
                    NoteBodyWarning {
                        id: "price:45%2C678%E5%86%86:2".into(),
                        message: price_warning("45,678円"),
                    },
                ],
                vec![
                    NoteBodyWarning {
                        id: "relative:-3.5%25:1".into(),
                        message: relative_warning("-3.5%"),
                    },
                    NoteBodyWarning {
                        id: "price:45%2C678%E5%86%86:1".into(),
                        message: price_warning("45,678円"),
                    },
                    NoteBodyWarning {
                        id: "price:45%2C678%E5%86%86:2".into(),
                        message: price_warning("45,678円"),
                    },
                ],
            ),
        );
    }

    #[test]
    fn scans_graph_label_warnings_with_locations() {
        let graph = GraphDef {
            id: "fictional-chart".into(),
            layout: Layout::Flow,
            title: None,
            nodes: vec![
                GraphNode {
                    id: "entry".into(),
                    label: "45,678円".into(),
                    r#ref: None,
                    value: None,
                    cite: None,
                    parent: None,
                    x: None,
                    y: None,
                },
                GraphNode {
                    id: "exit".into(),
                    label: "終点".into(),
                    r#ref: None,
                    value: None,
                    cite: None,
                    parent: None,
                    x: None,
                    y: None,
                },
            ],
            edges: vec![GraphEdge {
                source: "entry".into(),
                target: "exit".into(),
                label: Some("-3.5%".into()),
                value: None,
                cite: None,
            }],
        };

        let graphs = vec![graph];
        let preceding_warnings = scan_note_warnings_with_ids("close 45,678円", &[]);

        assert_eq!(
            (
                scan_note_warnings_with_ids("close 45,678円", &graphs),
                scan_graph_warnings_with_ids(&graphs, &preceding_warnings),
            ),
            (
                vec![
                    NoteBodyWarning {
                        id: "price:45%2C678%E5%86%86:1".into(),
                        message: price_warning("45,678円"),
                    },
                    NoteBodyWarning {
                        id: "price:45%2C678%E5%86%86:2".into(),
                        message: format!(
                            "graphs[0] (id=\"fictional-chart\").nodes[0] (id=\"entry\").label: {}",
                            price_warning("45,678円"),
                        ),
                    },
                    NoteBodyWarning {
                        id: "relative:-3.5%25:1".into(),
                        message: format!(
                            "graphs[0] (id=\"fictional-chart\").edges[0] (\"entry\" -> \"exit\").label: {}",
                            relative_warning("-3.5%"),
                        ),
                    },
                ],
                vec![
                    NoteBodyWarning {
                        id: "price:45%2C678%E5%86%86:1".into(),
                        message: format!(
                            "graphs[0] (id=\"fictional-chart\").nodes[0] (id=\"entry\").label: {}",
                            price_warning("45,678円"),
                        ),
                    },
                    NoteBodyWarning {
                        id: "relative:-3.5%25:1".into(),
                        message: format!(
                            "graphs[0] (id=\"fictional-chart\").edges[0] (\"entry\" -> \"exit\").label: {}",
                            relative_warning("-3.5%"),
                        ),
                    },
                ],
            ),
        );
    }
}
