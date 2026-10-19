use std::collections::HashMap;
use std::ops::Range;

use serde::Serialize;

use crate::note_graph::GraphDef;
use crate::note_reference::prose_segments;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum WarningKind {
    BarCount,
    Price,
    Relative,
}

#[derive(Debug)]
struct Finding {
    start: usize,
    value: String,
    kind: WarningKind,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct NoteBodyWarning {
    pub id: String,
    pub message: String,
}

pub fn scan_note_body_warnings_with_ids(body: &str) -> Vec<NoteBodyWarning> {
    scan_note_warnings_with_ids(body, &[])
}

pub fn scan_note_warnings_with_ids(body: &str, graphs: &[GraphDef]) -> Vec<NoteBodyWarning> {
    let mut occurrences = HashMap::<(WarningKind, String), usize>::new();
    let mut warnings = scan_text_with_ids(body, None, &mut occurrences);
    for (graph_index, graph) in graphs.iter().enumerate() {
        for (node_index, node) in graph.nodes.iter().enumerate() {
            let location = format!(
                "graphs[{graph_index}] (id={:?}).nodes[{node_index}] (id={:?}).label",
                graph.id, node.id
            );
            warnings.extend(scan_text_with_ids(
                &node.label,
                Some(&location),
                &mut occurrences,
            ));
        }
        for (edge_index, edge) in graph.edges.iter().enumerate() {
            let Some(label) = edge.label.as_deref() else {
                continue;
            };
            let location = format!(
                "graphs[{graph_index}] (id={:?}).edges[{edge_index}] ({:?} -> {:?}).label",
                graph.id, edge.source, edge.target
            );
            warnings.extend(scan_text_with_ids(label, Some(&location), &mut occurrences));
        }
    }
    warnings
}

fn scan_text_with_ids(
    text: &str,
    location: Option<&str>,
    occurrences: &mut HashMap<(WarningKind, String), usize>,
) -> Vec<NoteBodyWarning> {
    prose_segments(text)
        .into_iter()
        .flat_map(scan_prose_segment)
        .map(|finding| {
            let occurrence = occurrences
                .entry((finding.kind, finding.value.clone()))
                .and_modify(|count| *count += 1)
                .or_insert(1);
            let id = warning_id(finding.kind, &finding.value, *occurrence);
            let message = warning_message(finding.kind, &finding.value);
            let message = location
                .map(|location| format!("{location}: {message}"))
                .unwrap_or(message);
            NoteBodyWarning { id, message }
        })
        .collect()
}

fn warning_message(kind: WarningKind, value: &str) -> String {
    match kind {
        WarningKind::BarCount => format!(
            "ローソク足の本数「{value}」があります。本数ではなく、開始日と終了日で対象期間を記載してください。"
        ),
        WarningKind::Price => format!(
            "価格候補の数値「{value}」がリンク外にあります。株価であれば、銘柄・日付・項目を確認して `[[price:<id>@<date>:<field>]]` で参照してください。"
        ),
        WarningKind::Relative => format!(
            "相対表現「{value}」があります。計算結果を手入力せず、対象期間の値を `[[change:<id>@<start>..<end>:<field>]]` で示してください。概念上の目安ならそのままで構いません。"
        ),
    }
}

fn scan_prose_segment(segment: &str) -> Vec<Finding> {
    let number_ranges = number_ranges(segment);
    let relative_ranges = relative_expression_ranges(segment, &number_ranges);
    let bar_count_ranges = number_ranges
        .iter()
        .filter_map(|number| {
            bar_count_unit_end(segment, number.range.end)
                .map(|value_end| number.range.start..value_end)
        })
        .collect::<Vec<_>>();
    let bar_count_starts = bar_count_ranges
        .iter()
        .map(|range| range.start)
        .collect::<Vec<_>>();
    let mut findings = bar_count_ranges
        .into_iter()
        .map(|range| Finding {
            start: range.start,
            value: segment[range.clone()].to_string(),
            kind: WarningKind::BarCount,
        })
        .collect::<Vec<_>>();
    findings.extend(number_ranges.into_iter().filter_map(|number| {
        if relative_ranges
            .iter()
            .any(|range| overlaps(range, &number.range))
            || bar_count_starts.contains(&number.range.start)
            || !is_price_candidate(segment, &number)
        {
            return None;
        }

        let value_end = extend_price_unit(segment, number.range.end);
        Some(Finding {
            start: number.range.start,
            value: segment[number.range.start..value_end].to_string(),
            kind: WarningKind::Price,
        })
    }));
    findings.extend(relative_ranges.into_iter().map(|range| Finding {
        start: range.start,
        value: segment[range.clone()].to_string(),
        kind: WarningKind::Relative,
    }));
    findings.sort_unstable_by_key(|finding| finding.start);

    findings
}

fn warning_id(kind: WarningKind, value: &str, occurrence: usize) -> String {
    let kind = match kind {
        WarningKind::BarCount => "bar-count",
        WarningKind::Price => "price",
        WarningKind::Relative => "relative",
    };
    let mut encoded_value = String::new();
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            encoded_value.push(char::from(byte));
        } else {
            encoded_value.push('%');
            encoded_value.push(char::from(HEX[usize::from(byte >> 4)]));
            encoded_value.push(char::from(HEX[usize::from(byte & 0x0F)]));
        }
    }
    format!("{kind}:{encoded_value}:{occurrence}")
}

pub fn warning_id_group(id: &str) -> Option<&str> {
    let (group_id, occurrence) = id.rsplit_once(':')?;
    occurrence
        .parse::<usize>()
        .ok()
        .filter(|count| *count > 0)?;
    Some(group_id)
}

#[derive(Debug)]
struct NumberRange {
    range: Range<usize>,
    number_start: usize,
    digits: usize,
}

fn number_ranges(text: &str) -> Vec<NumberRange> {
    let mut ranges = Vec::new();
    let mut search_from = 0;

    while search_from < text.len() {
        let Some(digit_offset) =
            text[search_from..].find(|character: char| character.is_ascii_digit())
        else {
            break;
        };
        let digit_start = search_from + digit_offset;
        let range_start = number_range_start(text, digit_start);
        if starts_with_ascii_word_character_before(text, range_start) {
            search_from = digit_start + 1;
            continue;
        }

        let mut number_end = digit_start;
        let mut digits = 0;
        while let Some(character) = text[number_end..].chars().next() {
            if !character.is_ascii_digit() {
                break;
            }
            digits += 1;
            number_end += character.len_utf8();
        }
        while text[number_end..].starts_with(',') {
            let group_start = number_end + 1;
            let group_digits = text[group_start..]
                .chars()
                .take_while(|character| character.is_ascii_digit())
                .count();
            if group_digits != 3 {
                break;
            }
            number_end = group_start + 3;
            digits += 3;
        }
        if text[number_end..].starts_with('.') {
            let decimal_start = number_end + 1;
            let decimal_digits = text[decimal_start..]
                .chars()
                .take_while(|character| character.is_ascii_digit())
                .count();
            if decimal_digits > 0 {
                number_end = decimal_start + decimal_digits;
                digits += decimal_digits;
            }
        }

        ranges.push(NumberRange {
            range: range_start..number_end,
            number_start: digit_start,
            digits,
        });
        search_from = number_end;
    }

    ranges
}

fn number_range_start(text: &str, digit_start: usize) -> usize {
    match previous_char(text, digit_start) {
        Some((previous_start, '+' | '-' | '−' | '$' | '¥')) => previous_start,
        _ => digit_start,
    }
}

fn starts_with_ascii_word_character_before(text: &str, start: usize) -> bool {
    previous_char(text, start)
        .is_some_and(|(_, character)| character.is_ascii_alphabetic() || character == '_')
}

fn is_price_candidate(text: &str, number: &NumberRange) -> bool {
    if is_date_year(text, &number.range, number.digits) {
        return false;
    }
    number.digits >= 4
        || has_currency_prefix(text, number.number_start)
        || has_price_context(text, number.number_start)
        || has_price_unit(text, number.range.end)
}

fn is_date_year(text: &str, range: &Range<usize>, digits: usize) -> bool {
    if digits != 4 {
        return false;
    }
    let after = &text[range.end..];
    after.starts_with('年') || has_numeric_date_suffix(after)
}

fn has_numeric_date_suffix(after: &str) -> bool {
    let Some((separator_start, separator)) = after.char_indices().next() else {
        return false;
    };
    if !matches!(separator, '-' | '/' | '.') {
        return false;
    }

    let month_start = separator_start + separator.len_utf8();
    let month_len = after[month_start..]
        .chars()
        .take_while(|character| character.is_ascii_digit())
        .count();
    if !(1..=2).contains(&month_len) {
        return false;
    }
    let day_separator_start = month_start + month_len;
    if !after[day_separator_start..].starts_with(separator) {
        return false;
    }
    let day_start = day_separator_start + separator.len_utf8();
    (1..=2).contains(
        &after[day_start..]
            .chars()
            .take_while(|character| character.is_ascii_digit())
            .count(),
    )
}

fn has_currency_prefix(text: &str, number_start: usize) -> bool {
    if matches!(previous_char(text, number_start), Some((_, '$' | '¥'))) {
        return true;
    }
    let before = text[..number_start].trim_end().to_ascii_lowercase();
    ["usd", "jpy", "eur", "cny", "krw"]
        .into_iter()
        .any(|currency| before.ends_with(currency))
}

fn has_price_context(text: &str, number_start: usize) -> bool {
    let before = text[..number_start].trim_end();
    [
        "終値", "始値", "高値", "安値", "株価", "価格", "close", "open", "high", "low", "price",
    ]
    .into_iter()
    .any(|term| {
        if term.is_ascii() {
            before.to_ascii_lowercase().ends_with(term)
        } else {
            before.ends_with(term)
        }
    })
}

fn has_price_unit(text: &str, number_end: usize) -> bool {
    price_unit_end(text, number_end).is_some()
}

fn bar_count_unit_end(text: &str, number_end: usize) -> Option<usize> {
    let (unit, unit_end) = unit_after(text, number_end, &["営業日", "取引日", "足", "本"])?;
    if matches!(unit, "足" | "本")
        && text[unit_end..]
            .chars()
            .next()
            .is_some_and(is_cjk_ideograph)
    {
        return None;
    }
    if matches!(unit, "営業日" | "取引日") && text[unit_end..].starts_with('分') {
        return Some(unit_end + '分'.len_utf8());
    }
    Some(unit_end)
}

fn is_cjk_ideograph(character: char) -> bool {
    ('\u{4E00}'..='\u{9FFF}').contains(&character)
}

fn unit_after(
    text: &str,
    number_end: usize,
    units: &[&'static str],
) -> Option<(&'static str, usize)> {
    let suffix = &text[number_end..];
    let trimmed_suffix = suffix.trim_start();
    let unit_start = number_end + suffix.len() - trimmed_suffix.len();
    units.iter().copied().find_map(|unit| {
        trimmed_suffix
            .strip_prefix(unit)
            .map(|_| (unit, unit_start + unit.len()))
    })
}

fn extend_price_unit(text: &str, mut end: usize) -> usize {
    while let Some(unit_end) = price_unit_end(text, end) {
        end = unit_end;
    }
    end
}

fn price_unit_end(text: &str, number_end: usize) -> Option<usize> {
    unit_after(text, number_end, &["円", "万", "億", "千", "株", "ドル"])
        .map(|(_, unit_end)| unit_end)
}

fn relative_expression_ranges(text: &str, numbers: &[NumberRange]) -> Vec<Range<usize>> {
    let mut ranges = Vec::new();
    for (percent_start, percent) in text.char_indices() {
        if !matches!(percent, '%' | '％') {
            continue;
        }
        let Some((number_index, number)) = numbers.iter().enumerate().rev().find(|(_, number)| {
            number.range.end <= percent_start
                && text[number.range.end..percent_start]
                    .chars()
                    .all(char::is_whitespace)
        }) else {
            continue;
        };

        let mut expression_start = number.range.start;
        let mut current_index = number_index;
        while current_index > 0 {
            let previous = &numbers[current_index - 1];
            let current = &numbers[current_index];
            if is_price_candidate(text, previous)
                || !is_range_separator(&text[previous.range.end..current.number_start])
            {
                break;
            }
            expression_start = previous.range.start;
            current_index -= 1;
        }
        ranges.push(expression_start..percent_start + percent.len_utf8());
    }
    ranges
}

fn is_range_separator(between: &str) -> bool {
    matches!(
        between.trim_matches(|character: char| matches!(character, ' ' | '\t')),
        "+" | "-" | "−" | "–" | "—" | "~" | "〜" | "～"
    )
}

fn overlaps(left: &Range<usize>, right: &Range<usize>) -> bool {
    left.start < right.end && right.start < left.end
}

fn previous_char(text: &str, index: usize) -> Option<(usize, char)> {
    text[..index].char_indices().next_back()
}

#[cfg(test)]
mod tests {
    use indoc::indoc;
    use rstest::rstest;

    use crate::note_graph::{GraphDef, GraphEdge, GraphNode, Layout};

    use super::{NoteBodyWarning, scan_note_body_warnings_with_ids, scan_note_warnings_with_ids};

    #[rstest]
    #[case::price_and_relative_values(
        "close 12,345円 then -6.0%",
        vec![
            "価格候補の数値「12,345円」がリンク外にあります。株価であれば、銘柄・日付・項目を確認して `[[price:<id>@<date>:<field>]]` で参照してください。".to_string(),
            "相対表現「-6.0%」があります。計算結果を手入力せず、対象期間の値を `[[change:<id>@<start>..<end>:<field>]]` で示してください。概念上の目安ならそのままで構いません。".to_string(),
        ],
    )]
    #[case::small_price_with_spacing("for 950 円", vec![
        "価格候補の数値「950 円」がリンク外にあります。株価であれば、銘柄・日付・項目を確認して `[[price:<id>@<date>:<field>]]` で参照してください。".to_string(),
    ])]
    #[case::bare_four_digit_price_candidate("9876", vec![
        "価格候補の数値「9876」がリンク外にあります。株価であれば、銘柄・日付・項目を確認して `[[price:<id>@<date>:<field>]]` で参照してください。".to_string(),
    ])]
    #[case::bar_counts(
        "73足、18 本、53本のローソク足、61営業日、34 営業日分、26取引日、45 取引日分、10637本",
        vec![
            "ローソク足の本数「73足」があります。本数ではなく、開始日と終了日で対象期間を記載してください。".to_string(),
            "ローソク足の本数「18 本」があります。本数ではなく、開始日と終了日で対象期間を記載してください。".to_string(),
            "ローソク足の本数「53本」があります。本数ではなく、開始日と終了日で対象期間を記載してください。".to_string(),
            "ローソク足の本数「61営業日」があります。本数ではなく、開始日と終了日で対象期間を記載してください。".to_string(),
            "ローソク足の本数「34 営業日分」があります。本数ではなく、開始日と終了日で対象期間を記載してください。".to_string(),
            "ローソク足の本数「26取引日」があります。本数ではなく、開始日と終了日で対象期間を記載してください。".to_string(),
            "ローソク足の本数「45 取引日分」があります。本数ではなく、開始日と終了日で対象期間を記載してください。".to_string(),
            "ローソク足の本数「10637本」があります。本数ではなく、開始日と終了日で対象期間を記載してください。".to_string(),
        ],
    )]
    #[case::price_before_negative_change("終値 987654 -4.2%", vec![
        "価格候補の数値「987654」がリンク外にあります。株価であれば、銘柄・日付・項目を確認して `[[price:<id>@<date>:<field>]]` で参照してください。".to_string(),
        "相対表現「-4.2%」があります。計算結果を手入力せず、対象期間の値を `[[change:<id>@<start>..<end>:<field>]]` で示してください。概念上の目安ならそのままで構いません。".to_string(),
    ])]
    #[case::price_before_positive_change("close 86420 +3%", vec![
        "価格候補の数値「86420」がリンク外にあります。株価であれば、銘柄・日付・項目を確認して `[[price:<id>@<date>:<field>]]` で参照してください。".to_string(),
        "相対表現「+3%」があります。計算結果を手入力せず、対象期間の値を `[[change:<id>@<start>..<end>:<field>]]` で示してください。概念上の目安ならそのままで構いません。".to_string(),
    ])]
    #[case::unmatched_closing_bracket_is_not_a_link("値上がりは5.0% ](内容)", vec![
        "相対表現「5.0%」があります。計算結果を手入力せず、対象期間の値を `[[change:<id>@<start>..<end>:<field>]]` で示してください。概念上の目安ならそのままで構いません。".to_string(),
    ])]
    #[case::relative_range("日中2〜3%が通常", vec![
        "相対表現「2〜3%」があります。計算結果を手入力せず、対象期間の値を `[[change:<id>@<start>..<end>:<field>]]` で示してください。概念上の目安ならそのままで構いません。".to_string(),
    ])]
    #[case::spaced_relative_range("日中 2 ~ 3% が通常", vec![
        "相対表現「2 ~ 3%」があります。計算結果を手入力せず、対象期間の値を `[[change:<id>@<start>..<end>:<field>]]` で示してください。概念上の目安ならそのままで構いません。".to_string(),
    ])]
    #[case::hyphen_relative_range("2-3%", vec![
        "相対表現「2-3%」があります。計算結果を手入力せず、対象期間の値を `[[change:<id>@<start>..<end>:<field>]]` で示してください。概念上の目安ならそのままで構いません。".to_string(),
    ])]
    #[case::ordinary_date_is_not_a_price("2030-01-02", vec![])]
    #[case::price_followed_by_honjitsu(
        "終値 2,673 本日は反落",
        vec![
            "価格候補の数値「2,673」がリンク外にあります。株価であれば、銘柄・日付・項目を確認して `[[price:<id>@<date>:<field>]]` で参照してください。".to_string(),
        ],
    )]
    #[case::date_followed_by_ashimoto("9/6 足元では", vec![])]
    #[case::inline_link_url_is_not_scanned(
        "[123456円](https://news.example.com/articles/(2030)/12345678)",
        vec![
            "価格候補の数値「123456円」がリンク外にあります。株価であれば、銘柄・日付・項目を確認して `[[price:<id>@<date>:<field>]]` で参照してください。".to_string(),
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
            "価格候補の数値「5000円」がリンク外にあります。株価であれば、銘柄・日付・項目を確認して `[[price:<id>@<date>:<field>]]` で参照してください。".to_string(),
        ],
    )]
    #[case::japanese_comma_ends_bare_url(
        "https://example.com/a、次は5000円",
        vec![
            "価格候補の数値「5000円」がリンク外にあります。株価であれば、銘柄・日付・項目を確認して `[[price:<id>@<date>:<field>]]` で参照してください。".to_string(),
        ],
    )]
    #[case::japanese_text_ends_bare_url(
        "https://example.com/a次は5000円",
        vec![
            "価格候補の数値「5000円」がリンク外にあります。株価であれば、銘柄・日付・項目を確認して `[[price:<id>@<date>:<field>]]` で参照してください。".to_string(),
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
            scan_note_body_warnings_with_ids(body)
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
                scan_note_body_warnings_with_ids("45,678円、45,678円"),
                scan_note_body_warnings_with_ids("-3.5%、45,678円、45,678円"),
            ),
            (
                vec![
                    NoteBodyWarning {
                        id: "price:45%2C678%E5%86%86:1".into(),
                        message: "価格候補の数値「45,678円」がリンク外にあります。株価であれば、銘柄・日付・項目を確認して `[[price:<id>@<date>:<field>]]` で参照してください。".into(),
                    },
                    NoteBodyWarning {
                        id: "price:45%2C678%E5%86%86:2".into(),
                        message: "価格候補の数値「45,678円」がリンク外にあります。株価であれば、銘柄・日付・項目を確認して `[[price:<id>@<date>:<field>]]` で参照してください。".into(),
                    },
                ],
                vec![
                    NoteBodyWarning {
                        id: "relative:-3.5%25:1".into(),
                        message: "相対表現「-3.5%」があります。計算結果を手入力せず、対象期間の値を `[[change:<id>@<start>..<end>:<field>]]` で示してください。概念上の目安ならそのままで構いません。".into(),
                    },
                    NoteBodyWarning {
                        id: "price:45%2C678%E5%86%86:1".into(),
                        message: "価格候補の数値「45,678円」がリンク外にあります。株価であれば、銘柄・日付・項目を確認して `[[price:<id>@<date>:<field>]]` で参照してください。".into(),
                    },
                    NoteBodyWarning {
                        id: "price:45%2C678%E5%86%86:2".into(),
                        message: "価格候補の数値「45,678円」がリンク外にあります。株価であれば、銘柄・日付・項目を確認して `[[price:<id>@<date>:<field>]]` で参照してください。".into(),
                    },
                ],
            ),
        );
    }

    #[test]
    fn scans_graph_label_warnings_with_locations_and_shared_occurrence_ids() {
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

        assert_eq!(
            scan_note_warnings_with_ids("close 45,678円", &[graph]),
            vec![
                NoteBodyWarning {
                    id: "price:45%2C678%E5%86%86:1".into(),
                    message: "価格候補の数値「45,678円」がリンク外にあります。株価であれば、銘柄・日付・項目を確認して `[[price:<id>@<date>:<field>]]` で参照してください。".into(),
                },
                NoteBodyWarning {
                    id: "price:45%2C678%E5%86%86:2".into(),
                    message: "graphs[0] (id=\"fictional-chart\").nodes[0] (id=\"entry\").label: 価格候補の数値「45,678円」がリンク外にあります。株価であれば、銘柄・日付・項目を確認して `[[price:<id>@<date>:<field>]]` で参照してください。".into(),
                },
                NoteBodyWarning {
                    id: "relative:-3.5%25:1".into(),
                    message: "graphs[0] (id=\"fictional-chart\").edges[0] (\"entry\" -> \"exit\").label: 相対表現「-3.5%」があります。計算結果を手入力せず、対象期間の値を `[[change:<id>@<start>..<end>:<field>]]` で示してください。概念上の目安ならそのままで構いません。".into(),
                },
            ],
        );
    }
}
