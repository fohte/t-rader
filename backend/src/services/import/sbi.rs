//! SBI 証券「国内株式 取引履歴」CSV のパーサ。
//!
//! Web の「口座管理 → 取引履歴」から DL される CSV を取込む。
//! ref: <https://search.sbisec.co.jp/v2/popwin/help/cx/order_history.html>
//!
//! 想定フォーマット (Shift_JIS、ヘッダの位置はファイル先頭とは限らない):
//!
//! ```text
//! "約定日","銘柄","銘柄コード","市場","取引","期限","預り","課税",
//!   "数量[株]","単価[円]","受渡日","手数料[円]","税額[円]","受渡金額/(決済対価)[円]"
//! "2026/01/15","トヨタ自動車","7203","東証P","株式現物買","-","特定","-",
//!   "100","2,500","2026/1/19","55","6","250,061"
//! ```
//!
//! - 「取引」列に "買" を含めば buy、"売" を含めば sell
//! - 数値カラムは 3 桁カンマ区切り、Decimal でパース
//! - 投信 / 米国株は対象外 (MVP)。「取引」列が "株式現物" を含まない行は skip

use chrono::NaiveDate;
use encoding_rs::SHIFT_JIS;
use rust_decimal::Decimal;
use std::str::FromStr;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SbiTradeRow {
    /// 0-based 行番号。ヘッダを含む元 CSV 上での位置。
    pub row_index: usize,
    pub date: NaiveDate,
    /// 銘柄コード (例: 7203)
    pub symbol: String,
    /// 銘柄名 (stock 未登録時の自動作成用)
    pub stock_name: String,
    /// "buy" | "sell"
    pub side: String,
    pub qty: Decimal,
    pub price: Decimal,
    pub fee: Decimal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SbiParseIssue {
    pub row_index: usize,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SbiParseResult {
    pub rows: Vec<SbiTradeRow>,
    pub issues: Vec<SbiParseIssue>,
}

#[derive(Debug, thiserror::Error)]
pub enum SbiParseError {
    #[error("CSV ヘッダ行 ('約定日' を含む行) が見つかりません")]
    HeaderNotFound,
    #[error("CSV の読み込みに失敗しました: {0}")]
    Csv(String),
}

const HEADER_DATE: &str = "約定日";
const HEADER_NAME: &str = "銘柄";
const HEADER_CODE: &str = "銘柄コード";
const HEADER_KIND: &str = "取引";
const HEADER_QTY: &str = "数量";
const HEADER_PRICE: &str = "単価";
const HEADER_FEE: &str = "手数料";

/// 生バイト列 (Shift_JIS or UTF-8) をパースする。
pub fn parse_bytes(bytes: &[u8]) -> Result<SbiParseResult, SbiParseError> {
    let text = decode_csv(bytes);
    parse_text(&text)
}

/// UTF-8 (BOM 許容) を先に試し、不正なら Shift_JIS lossy decode にフォールバックする。
fn decode_csv(bytes: &[u8]) -> String {
    let without_bom = bytes.strip_prefix(b"\xef\xbb\xbf").unwrap_or(bytes);
    if let Ok(s) = std::str::from_utf8(without_bom) {
        return s.to_string();
    }
    let (decoded, _, _) = SHIFT_JIS.decode(bytes);
    decoded.into_owned()
}

fn parse_text(text: &str) -> Result<SbiParseResult, SbiParseError> {
    // SBI CSV は先頭にメタ行 (タイトル、空行等) が入ることがある。
    // 「約定日」を含む行を検出してそこからを CSV ヘッダとみなす。
    let mut lines = text.lines().enumerate();
    let mut header_line: Option<(usize, &str)> = None;
    for (idx, line) in lines.by_ref() {
        if line.contains(HEADER_DATE) && line.contains(HEADER_CODE) {
            header_line = Some((idx, line));
            break;
        }
    }
    let (header_idx, header_line) = header_line.ok_or(SbiParseError::HeaderNotFound)?;

    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(true)
        .from_reader(
            // ヘッダ + 残り行を再結合して CSV reader に流す
            std::io::Cursor::new(format!(
                "{}\n{}",
                header_line,
                text.lines()
                    .skip(header_idx + 1)
                    .collect::<Vec<_>>()
                    .join("\n"),
            )),
        );

    let headers = reader
        .headers()
        .map_err(|e| SbiParseError::Csv(e.to_string()))?
        .clone();
    let col = ColumnIndex::resolve(&headers)?;

    let mut result = SbiParseResult::default();
    for (offset, record) in reader.records().enumerate() {
        let row_index = header_idx + 1 + offset;
        let record = match record {
            Ok(r) => r,
            Err(e) => {
                result.issues.push(SbiParseIssue {
                    row_index,
                    message: format!("CSV 行の読み込みに失敗: {e}"),
                });
                continue;
            }
        };
        if is_blank(&record) {
            continue;
        }
        match parse_record(row_index, &col, &record) {
            Ok(Some(row)) => result.rows.push(row),
            Ok(None) => {} // 国内株現物以外は skip
            Err(msg) => result.issues.push(SbiParseIssue {
                row_index,
                message: msg,
            }),
        }
    }
    Ok(result)
}

struct ColumnIndex {
    date: usize,
    name: usize,
    code: usize,
    kind: usize,
    qty: usize,
    price: usize,
    fee: usize,
}

impl ColumnIndex {
    fn resolve(headers: &csv::StringRecord) -> Result<Self, SbiParseError> {
        let find = |needle: &str| -> Result<usize, SbiParseError> {
            headers
                .iter()
                .position(|h| h.contains(needle))
                .ok_or_else(|| SbiParseError::Csv(format!("ヘッダ '{needle}' が見つかりません")))
        };
        Ok(Self {
            date: find(HEADER_DATE)?,
            name: find(HEADER_NAME)?,
            code: find(HEADER_CODE)?,
            kind: find(HEADER_KIND)?,
            qty: find(HEADER_QTY)?,
            price: find(HEADER_PRICE)?,
            fee: find(HEADER_FEE)?,
        })
    }
}

fn is_blank(record: &csv::StringRecord) -> bool {
    record.iter().all(|f| f.trim().is_empty())
}

fn parse_record(
    row_index: usize,
    col: &ColumnIndex,
    record: &csv::StringRecord,
) -> Result<Option<SbiTradeRow>, String> {
    let get = |i: usize| -> &str { record.get(i).unwrap_or("").trim() };

    let kind = get(col.kind);
    // MVP: 「株式現物」のみサポート。信用 / 投信 / 米国株はここで skip。
    if !kind.contains("株式現物") {
        return Ok(None);
    }
    let side = if kind.contains('買') {
        "buy"
    } else if kind.contains('売') {
        "sell"
    } else {
        return Err(format!("売買区分を判定できません: '{kind}'"));
    };

    let date = parse_date(get(col.date))?;
    let symbol = get(col.code).to_string();
    if symbol.is_empty() {
        return Err("銘柄コードが空です".into());
    }
    let stock_name = get(col.name).to_string();
    let qty = parse_decimal(get(col.qty)).map_err(|e| format!("数量のパース失敗: {e}"))?;
    let price = parse_decimal(get(col.price)).map_err(|e| format!("単価のパース失敗: {e}"))?;
    let fee = parse_decimal(get(col.fee)).unwrap_or(Decimal::ZERO);

    Ok(Some(SbiTradeRow {
        row_index,
        date,
        symbol,
        stock_name,
        side: side.to_string(),
        qty,
        price,
        fee,
    }))
}

fn parse_date(raw: &str) -> Result<NaiveDate, String> {
    // SBI は "2026/1/15" / "2026/01/15" / "2026-01-15" の混在があり得るので正規化する
    let normalized = raw.replace('-', "/");
    NaiveDate::parse_from_str(&normalized, "%Y/%m/%d")
        .map_err(|e| format!("日付のパース失敗 '{raw}': {e}"))
}

fn parse_decimal(raw: &str) -> Result<Decimal, String> {
    let cleaned: String = raw
        .chars()
        .filter(|c| !matches!(c, ',' | '円' | '株' | ' ' | '\u{3000}'))
        .collect();
    if cleaned.is_empty() || cleaned == "-" {
        return Ok(Decimal::ZERO);
    }
    Decimal::from_str(&cleaned).map_err(|e| format!("'{raw}' -> '{cleaned}': {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        match NaiveDate::from_ymd_opt(y, m, day) {
            Some(v) => v,
            None => unreachable!("invalid fixture date {y}/{m}/{day}"),
        }
    }

    const FIXTURE: &str = indoc::indoc! {r#"
        "口座管理 取引履歴 国内株式"

        "約定日","銘柄","銘柄コード","市場","取引","期限","預り","課税","数量[株]","単価[円]","受渡日","手数料[円]","税額[円]","受渡金額/(決済対価)[円]"
        "2026/01/15","トヨタ自動車","7203","東証P","株式現物買","-","特定","-","100","2,500","2026/1/19","55","6","250,061"
        "2026/02/01","ソニーグループ","6758","東証P","株式現物売","-","特定","-","50","18,200","2026/2/5","275","27","909,698"
        "2026/03/10","信用銘柄","9999","東証P","株式信用新規買","6ヶ月","特定","-","100","1,000","2026/3/12","99","9","100,000"
    "#};

    fn expected_fixture_result() -> SbiParseResult {
        SbiParseResult {
            rows: vec![
                SbiTradeRow {
                    row_index: 3,
                    date: d(2026, 1, 15),
                    symbol: "7203".into(),
                    stock_name: "トヨタ自動車".into(),
                    side: "buy".into(),
                    qty: Decimal::from(100),
                    price: Decimal::from(2500),
                    fee: Decimal::from(55),
                },
                SbiTradeRow {
                    row_index: 4,
                    date: d(2026, 2, 1),
                    symbol: "6758".into(),
                    stock_name: "ソニーグループ".into(),
                    side: "sell".into(),
                    qty: Decimal::from(50),
                    price: Decimal::from(18200),
                    fee: Decimal::from(275),
                },
            ],
            issues: vec![],
        }
    }

    #[test]
    fn parse_text_handles_preamble_and_only_keeps_genbutsu() {
        assert_eq!(
            parse_text(FIXTURE).expect("parse"),
            expected_fixture_result()
        );
    }

    #[test]
    fn parse_bytes_decodes_shift_jis() {
        let (encoded, _, _) = SHIFT_JIS.encode(FIXTURE);
        assert_eq!(
            parse_bytes(&encoded).expect("parse"),
            expected_fixture_result(),
        );
    }

    #[test]
    fn parse_text_returns_error_when_header_missing() {
        let input = indoc::indoc! {"
            no header here
            1,2,3
        "};
        let err = parse_text(input).unwrap_err();
        assert!(matches!(err, SbiParseError::HeaderNotFound));
    }

    #[rstest]
    #[case::comma_separated("2,500", Decimal::from(2500))]
    #[case::with_unit("100 株", Decimal::from(100))]
    #[case::dash_means_zero("-", Decimal::ZERO)]
    #[case::empty("", Decimal::ZERO)]
    fn parse_decimal_strips_noise(#[case] input: &str, #[case] expected: Decimal) {
        assert_eq!(parse_decimal(input).expect("parse"), expected);
    }

    #[rstest]
    #[case::slash("2026/01/15", d(2026, 1, 15))]
    #[case::slash_short_month("2026/1/15", d(2026, 1, 15))]
    #[case::dash("2026-01-15", d(2026, 1, 15))]
    fn parse_date_accepts_multiple_formats(#[case] input: &str, #[case] expected: NaiveDate) {
        assert_eq!(parse_date(input).expect("parse"), expected);
    }
}
