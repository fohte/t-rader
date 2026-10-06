use std::ops::Range;

use crate::note_reference::prose_segments;

#[derive(Debug, Clone, Copy)]
enum WarningKind {
    Price,
    Relative,
}

#[derive(Debug)]
struct Finding {
    start: usize,
    value: String,
    kind: WarningKind,
}

pub fn scan_note_body_warnings(body: &str) -> Vec<String> {
    prose_segments(body)
        .into_iter()
        .flat_map(scan_prose_segment)
        .collect()
}

fn scan_prose_segment(segment: &str) -> Vec<String> {
    let number_ranges = number_ranges(segment);
    let relative_ranges = relative_expression_ranges(segment, &number_ranges);
    let mut findings = number_ranges
        .into_iter()
        .filter_map(|number| {
            if relative_ranges
                .iter()
                .any(|range| overlaps(range, &number.range))
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
        })
        .collect::<Vec<_>>();
    findings.extend(relative_ranges.into_iter().map(|range| Finding {
        start: range.start,
        value: segment[range.clone()].to_string(),
        kind: WarningKind::Relative,
    }));
    findings.sort_unstable_by_key(|finding| finding.start);

    findings
        .into_iter()
        .map(|finding| match finding.kind {
            WarningKind::Price => format!(
                "価格候補の数値「{}」がリンク外にあります。株価であれば、銘柄・日付・項目を確認して `[[price:<id>@<date>:<field>]]` で参照してください。",
                finding.value
            ),
            WarningKind::Relative => format!(
                "相対表現「{}」があります。計算結果を手入力せず、対象期間の値を `[[change:<id>@<start>..<end>:<field>]]` で示してください。概念上の目安ならそのままで構いません。",
                finding.value
            ),
        })
        .collect()
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

fn extend_price_unit(text: &str, mut end: usize) -> usize {
    while let Some(unit_end) = price_unit_end(text, end) {
        end = unit_end;
    }
    end
}

fn price_unit_end(text: &str, number_end: usize) -> Option<usize> {
    let suffix = &text[number_end..];
    let trimmed_suffix = suffix.trim_start();
    let unit_start = number_end + suffix.len() - trimmed_suffix.len();
    ["円", "万", "億", "千", "株", "ドル"]
        .into_iter()
        .find(|unit| trimmed_suffix.starts_with(unit))
        .map(|unit| unit_start + unit.len())
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

    use super::scan_note_body_warnings;

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
    #[case::price_before_negative_change("終値 987654 -4.2%", vec![
        "価格候補の数値「987654」がリンク外にあります。株価であれば、銘柄・日付・項目を確認して `[[price:<id>@<date>:<field>]]` で参照してください。".to_string(),
        "相対表現「-4.2%」があります。計算結果を手入力せず、対象期間の値を `[[change:<id>@<start>..<end>:<field>]]` で示してください。概念上の目安ならそのままで構いません。".to_string(),
    ])]
    #[case::price_before_positive_change("close 86420 +3%", vec![
        "価格候補の数値「86420」がリンク外にあります。株価であれば、銘柄・日付・項目を確認して `[[price:<id>@<date>:<field>]]` で参照してください。".to_string(),
        "相対表現「+3%」があります。計算結果を手入力せず、対象期間の値を `[[change:<id>@<start>..<end>:<field>]]` で示してください。概念上の目安ならそのままで構いません。".to_string(),
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
    #[case::links_are_not_scanned(
        "[[price:fictional-code@2030-01-02:close]] [[change:fictional-code@2030-01-02..2030-01-03:close]]",
        vec![],
    )]
    #[case::code_is_not_scanned(
        indoc! {"
            `12,345円 -6.0%`

            ```text
            34,567円 +2.0%
            ```
        "},
        vec![],
    )]
    fn scans_only_unlinked_price_and_relative_values(
        #[case] body: &str,
        #[case] expected: Vec<String>,
    ) {
        assert_eq!(scan_note_body_warnings(body), expected);
    }
}
