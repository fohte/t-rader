use std::ops::Range;

use super::ranges_overlap;
use super::types::{MarkdownLintContext, RuleFinding};
use crate::note_reference::prose_segment_ranges;

const PRICE_WARNING_SUFFIX: &str = "がリンク外にあります。株価であれば、銘柄・日付・項目を確認して `[[price:<id>@<date>:<field>]]` で参照してください。株価ではない値ならそのまま残し、必要な理由を `warning_dispositions` に記載して警告を無視してください。";
const RELATIVE_WARNING_SUFFIX: &str = "があります。株価の騰落率であれば、計算結果を手入力せず、対象期間の値を `[[change:<id>@<start>..<end>:<field>]]` で示してください。株価の騰落率ではない値はそのまま残し、必要な理由を `warning_dispositions` に記載して警告を無視してください。";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MarketNumericKind {
    BarCount,
    Price,
    Relative,
}

impl MarketNumericKind {
    pub const fn rule_id(self) -> &'static str {
        match self {
            Self::BarCount => "bar-count",
            Self::Price => "price",
            Self::Relative => "relative",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarketNumericFinding {
    pub kind: MarketNumericKind,
    pub key: String,
    pub range: Range<usize>,
    pub message: String,
}

pub(crate) fn findings_of_kind(
    context: &MarkdownLintContext<'_>,
    kind: MarketNumericKind,
) -> Vec<RuleFinding> {
    context
        .market_numeric_findings
        .iter()
        .filter(|finding| finding.kind == kind)
        .map(RuleFinding::from_market_numeric)
        .collect()
}

#[derive(Debug)]
struct Finding {
    start: usize,
    value: String,
    kind: MarketNumericKind,
}

pub(crate) fn scan_market_numeric_source(source: &str) -> Vec<MarketNumericFinding> {
    prose_segment_ranges(source)
        .into_iter()
        .flat_map(|range| scan_prose_segment(source, range))
        .collect()
}

pub(crate) fn scan_prose_segment(
    source: &str,
    source_range: Range<usize>,
) -> Vec<MarketNumericFinding> {
    let segment = &source[source_range.clone()];
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
            kind: MarketNumericKind::BarCount,
        })
        .collect::<Vec<_>>();
    findings.extend(number_ranges.into_iter().filter_map(|number| {
        if relative_ranges
            .iter()
            .any(|range| ranges_overlap(range, &number.range))
            || bar_count_starts.contains(&number.range.start)
            || !is_price_candidate(segment, &number)
        {
            return None;
        }

        let value_end = extend_price_unit(segment, number.range.end);
        Some(Finding {
            start: number.range.start,
            value: segment[number.range.start..value_end].to_string(),
            kind: MarketNumericKind::Price,
        })
    }));
    findings.extend(relative_ranges.into_iter().map(|range| Finding {
        start: range.start,
        value: segment[range.clone()].to_string(),
        kind: MarketNumericKind::Relative,
    }));
    findings.sort_unstable_by_key(|finding| finding.start);

    findings
        .into_iter()
        .map(|finding| MarketNumericFinding {
            kind: finding.kind,
            key: finding.value.clone(),
            range: source_range.start + finding.start
                ..source_range.start + finding.start + finding.value.len(),
            message: warning_message(finding.kind, &finding.value),
        })
        .collect()
}

fn warning_message(kind: MarketNumericKind, value: &str) -> String {
    match kind {
        MarketNumericKind::BarCount => format!(
            "ローソク足の本数「{value}」があります。本数ではなく、開始日と終了日で対象期間を記載してください。"
        ),
        MarketNumericKind::Price => format!("価格候補の数値「{value}」{PRICE_WARNING_SUFFIX}"),
        MarketNumericKind::Relative => format!("相対表現「{value}」{RELATIVE_WARNING_SUFFIX}"),
    }
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
    if is_date_year(text, &number.range, number.digits)
        || has_non_price_scaled_counter(text, number.range.end)
    {
        return false;
    }
    number.digits >= 4
        || has_currency_prefix(text, number.number_start)
        || has_price_context(text, number.number_start)
        || has_price_unit(text, number.range.end)
}

fn has_non_price_scaled_counter(text: &str, number_end: usize) -> bool {
    let Some((_, scale_end)) = unit_after(text, number_end, &["万", "億", "千"]) else {
        return false;
    };
    let suffix = &text[scale_end..];
    let Some((unit, rest)) = [
        "件",
        "人",
        "回",
        "台",
        "社",
        "個",
        "本",
        "名",
        "枚",
        "冊",
        "匹",
        "頭",
        "羽",
        "軒",
        "戸",
        "棟",
        "株",
        "口",
        "組",
        "世帯",
        "店舗",
        "倍",
        "ポイント",
        "年",
        "月",
        "日",
        "時間",
        "分",
        "秒",
    ]
    .into_iter()
    .find_map(|unit| suffix.strip_prefix(unit).map(|rest| (unit, rest))) else {
        return false;
    };
    unit.chars().count() > 1 || !rest.chars().next().is_some_and(is_cjk_ideograph)
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

fn previous_char(text: &str, index: usize) -> Option<(usize, char)> {
    text[..index].char_indices().next_back()
}
