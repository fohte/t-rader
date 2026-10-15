use chrono::NaiveDate;

use crate::note_reference::{NoteTokenValidationError, tokens_outside_code};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PriceReferenceField {
    Open,
    High,
    Low,
    Close,
    Volume,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValuationReferenceField {
    Per,
    FwdPer,
    Pbr,
    Roe,
    FwdRoe,
    MktCap,
    Eps,
    FwdEps,
    Bps,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NotePriceReference {
    Price {
        instrument_id: String,
        date: NaiveDate,
        field: PriceReferenceField,
    },
    Change {
        instrument_id: String,
        start: NaiveDate,
        end: NaiveDate,
        field: PriceReferenceField,
    },
    Valuation {
        symbol: String,
        date: NaiveDate,
        field: ValuationReferenceField,
    },
    Indicator {
        indicator_id: String,
        date: NaiveDate,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotePriceReferenceToken {
    pub token: String,
    pub reference: NotePriceReference,
}

pub fn extract_note_price_references(
    body: &str,
) -> Result<Vec<NotePriceReferenceToken>, Vec<NoteTokenValidationError>> {
    let mut references = Vec::new();
    let mut errors = Vec::new();

    for token in tokens_outside_code(body)
        .into_iter()
        .filter(|token| is_value_reference_kind(token.inner))
    {
        let token_text = body[token.start..token.end].to_string();
        match parse_note_price_reference(token.inner) {
            Ok(reference) => references.push(NotePriceReferenceToken {
                token: token_text,
                reference,
            }),
            Err(reason) => errors.push(NoteTokenValidationError::BodyToken {
                token: token_text,
                reason: reason.to_string(),
            }),
        }
    }

    if errors.is_empty() {
        Ok(references)
    } else {
        Err(errors)
    }
}

fn is_value_reference_kind(token: &str) -> bool {
    let Some((kind, content)) = token.split_once(':') else {
        return false;
    };
    matches!(kind, "price" | "change" | "valuation")
        || (kind == "indicator" && content.contains('@'))
}

fn parse_note_price_reference(token: &str) -> Result<NotePriceReference, &'static str> {
    let (kind, content) = token
        .split_once(':')
        .ok_or("`price:<id>@<date>:<field>` または `change:<id>@<start>..<end>:<field>` を指定してください")?;
    match kind {
        "price" => {
            let (instrument_id, dates, field) = parse_instrument_date_field(
                content,
                "項目を `open`, `high`, `low`, `close`, `volume` のいずれかで指定してください",
            )?;
            Ok(NotePriceReference::Price {
                instrument_id,
                field: parse_price_field(field)?,
                date: parse_date(dates)?,
            })
        }
        "change" => {
            let (instrument_id, dates, field) = parse_instrument_date_field(
                content,
                "項目を `open`, `high`, `low`, `close`, `volume` のいずれかで指定してください",
            )?;
            let field = parse_price_field(field)?;
            let (start, end) = dates
                .split_once("..")
                .ok_or("期間を `YYYY-MM-DD..YYYY-MM-DD` で指定してください")?;
            if end.contains("..") {
                return Err("期間は 2 つの日付で指定してください");
            }
            let start = parse_date(start)?;
            let end = parse_date(end)?;
            if start >= end {
                return Err("期間の開始日は終了日より前にしてください");
            }
            Ok(NotePriceReference::Change {
                instrument_id: instrument_id.to_string(),
                start,
                end,
                field,
            })
        }
        "valuation" => {
            let (symbol, dates, field) = parse_instrument_date_field(
                content,
                "valuation の項目は `per`, `fwd_per`, `pbr`, `roe`, `fwd_roe`, `mkt_cap`, `eps`, `fwd_eps`, `bps` のいずれかで指定してください",
            )?;
            Ok(NotePriceReference::Valuation {
                symbol,
                field: parse_valuation_field(field)?,
                date: parse_date(dates)?,
            })
        }
        "indicator" => {
            let (indicator_id, date) = content
                .rsplit_once('@')
                .ok_or("指標 ID と日付を `@` で区切ってください")?;
            if !is_valid_instrument_id(indicator_id) {
                return Err("指標 ID を空にせず、空白や `@` を含めないでください");
            }
            Ok(NotePriceReference::Indicator {
                indicator_id: indicator_id.to_string(),
                date: parse_date(date)?,
            })
        }
        _ => Err("価格参照は `price` または `change` を指定してください"),
    }
}

fn parse_instrument_date_field<'a>(
    content: &'a str,
    missing_field_error: &'static str,
) -> Result<(String, &'a str, &'a str), &'static str> {
    let (instrument_and_date, field) = content.rsplit_once(':').ok_or(missing_field_error)?;
    let (instrument_id, date) = instrument_and_date
        .rsplit_once('@')
        .ok_or("銘柄 ID と日付を `@` で区切ってください")?;
    if !is_valid_instrument_id(instrument_id) {
        return Err("銘柄 ID を空にせず、空白や `@` を含めないでください");
    }
    Ok((instrument_id.to_string(), date, field))
}

fn parse_date(value: &str) -> Result<NaiveDate, &'static str> {
    let bytes = value.as_bytes();
    if bytes.len() != 10
        || bytes[4] != b'-'
        || bytes[7] != b'-'
        || bytes
            .iter()
            .enumerate()
            .any(|(index, byte)| index != 4 && index != 7 && !byte.is_ascii_digit())
    {
        return Err("日付を `YYYY-MM-DD` で指定してください");
    }
    let date = NaiveDate::parse_from_str(value, "%Y-%m-%d")
        .map_err(|_| "日付を `YYYY-MM-DD` で指定してください")?;
    Ok(date)
}

fn parse_price_field(value: &str) -> Result<PriceReferenceField, &'static str> {
    match value {
        "open" => Ok(PriceReferenceField::Open),
        "high" => Ok(PriceReferenceField::High),
        "low" => Ok(PriceReferenceField::Low),
        "close" => Ok(PriceReferenceField::Close),
        "volume" => Ok(PriceReferenceField::Volume),
        _ => Err("項目を `open`, `high`, `low`, `close`, `volume` のいずれかで指定してください"),
    }
}

fn parse_valuation_field(value: &str) -> Result<ValuationReferenceField, &'static str> {
    match value {
        "per" => Ok(ValuationReferenceField::Per),
        "fwd_per" => Ok(ValuationReferenceField::FwdPer),
        "pbr" => Ok(ValuationReferenceField::Pbr),
        "roe" => Ok(ValuationReferenceField::Roe),
        "fwd_roe" => Ok(ValuationReferenceField::FwdRoe),
        "mkt_cap" => Ok(ValuationReferenceField::MktCap),
        "eps" => Ok(ValuationReferenceField::Eps),
        "fwd_eps" => Ok(ValuationReferenceField::FwdEps),
        "bps" => Ok(ValuationReferenceField::Bps),
        _ => Err(
            "valuation の項目は `per`, `fwd_per`, `pbr`, `roe`, `fwd_roe`, `mkt_cap`, `eps`, `fwd_eps`, `bps` のいずれかで指定してください",
        ),
    }
}

fn is_valid_instrument_id(value: &str) -> bool {
    !value.is_empty()
        && value
            .chars()
            .all(|character| !character.is_whitespace() && character != '@')
}

#[cfg(test)]
mod tests {
    use indoc::indoc;
    use rstest::rstest;

    use super::{
        NotePriceReference, NotePriceReferenceToken, PriceReferenceField, ValuationReferenceField,
        extract_note_price_references,
    };
    use crate::note_reference::NoteTokenValidationError;

    #[rstest]
    #[case::price_close("[[price:fictional-code@2030-01-02:close]]", NotePriceReference::Price { instrument_id: "fictional-code".into(), date: "2030-01-02".parse().expect("date"), field: PriceReferenceField::Close })]
    #[case::foreign_stock_id("[[price:US:FICTIONAL-A@2030-01-02:open]]", NotePriceReference::Price { instrument_id: "US:FICTIONAL-A".into(), date: "2030-01-02".parse().expect("date"), field: PriceReferenceField::Open })]
    #[case::price_high("[[price:fictional-code@2030-01-02:high]]", NotePriceReference::Price { instrument_id: "fictional-code".into(), date: "2030-01-02".parse().expect("date"), field: PriceReferenceField::High })]
    #[case::price_low("[[price:fictional-code@2030-01-02:low]]", NotePriceReference::Price { instrument_id: "fictional-code".into(), date: "2030-01-02".parse().expect("date"), field: PriceReferenceField::Low })]
    #[case::change_volume("[[change:fictional-code@2030-01-02..2030-01-03:volume]]", NotePriceReference::Change { instrument_id: "fictional-code".into(), start: "2030-01-02".parse().expect("start date"), end: "2030-01-03".parse().expect("end date"), field: PriceReferenceField::Volume })]
    #[case::valuation_per("[[valuation:fictional-code@2030-01-02:per]]", NotePriceReference::Valuation { symbol: "fictional-code".into(), date: "2030-01-02".parse().expect("date"), field: ValuationReferenceField::Per })]
    #[case::valuation_fwd_per("[[valuation:fictional-code@2030-01-02:fwd_per]]", NotePriceReference::Valuation { symbol: "fictional-code".into(), date: "2030-01-02".parse().expect("date"), field: ValuationReferenceField::FwdPer })]
    #[case::valuation_pbr("[[valuation:fictional-code@2030-01-02:pbr]]", NotePriceReference::Valuation { symbol: "fictional-code".into(), date: "2030-01-02".parse().expect("date"), field: ValuationReferenceField::Pbr })]
    #[case::valuation_roe("[[valuation:fictional-code@2030-01-02:roe]]", NotePriceReference::Valuation { symbol: "fictional-code".into(), date: "2030-01-02".parse().expect("date"), field: ValuationReferenceField::Roe })]
    #[case::valuation_fwd_roe("[[valuation:fictional-code@2030-01-02:fwd_roe]]", NotePriceReference::Valuation { symbol: "fictional-code".into(), date: "2030-01-02".parse().expect("date"), field: ValuationReferenceField::FwdRoe })]
    #[case::valuation_market_cap("[[valuation:fictional-code@2030-01-02:mkt_cap]]", NotePriceReference::Valuation { symbol: "fictional-code".into(), date: "2030-01-02".parse().expect("date"), field: ValuationReferenceField::MktCap })]
    #[case::valuation_eps("[[valuation:fictional-code@2030-01-02:eps]]", NotePriceReference::Valuation { symbol: "fictional-code".into(), date: "2030-01-02".parse().expect("date"), field: ValuationReferenceField::Eps })]
    #[case::valuation_forward_eps("[[valuation:fictional-code@2030-01-02:fwd_eps]]", NotePriceReference::Valuation { symbol: "fictional-code".into(), date: "2030-01-02".parse().expect("date"), field: ValuationReferenceField::FwdEps })]
    #[case::valuation_bps("[[valuation:fictional-code@2030-01-02:bps]]", NotePriceReference::Valuation { symbol: "fictional-code".into(), date: "2030-01-02".parse().expect("date"), field: ValuationReferenceField::Bps })]
    #[case::indicator_observation("[[indicator:demo-indicator@2030-01-02]]", NotePriceReference::Indicator { indicator_id: "demo-indicator".into(), date: "2030-01-02".parse().expect("date") })]
    fn parses_price_reference_links(#[case] body: &str, #[case] reference: NotePriceReference) {
        assert_eq!(
            extract_note_price_references(body),
            Ok(vec![NotePriceReferenceToken {
                token: body.to_string(),
                reference,
            }]),
        );
    }

    #[rstest]
    #[case::unknown_field(
        "[[price:fictional-code@2030-01-02:adjusted]]",
        "項目を `open`, `high`, `low`, `close`, `volume` のいずれかで指定してください"
    )]
    #[case::malformed_date(
        "[[price:fictional-code@2030-1-2:close]]",
        "日付を `YYYY-MM-DD` で指定してください"
    )]
    #[case::invalid_range(
        "[[change:fictional-code@2030-01-03..2030-01-02:close]]",
        "期間の開始日は終了日より前にしてください"
    )]
    #[case::missing_endpoint(
        "[[change:fictional-code@2030-01-02..:close]]",
        "日付を `YYYY-MM-DD` で指定してください"
    )]
    #[case::unknown_valuation_field(
        "[[valuation:fictional-code@2030-01-02:adjusted]]",
        "valuation の項目は `per`, `fwd_per`, `pbr`, `roe`, `fwd_roe`, `mkt_cap`, `eps`, `fwd_eps`, `bps` のいずれかで指定してください"
    )]
    #[case::malformed_valuation_date(
        "[[valuation:fictional-code@2030-1-2:per]]",
        "日付を `YYYY-MM-DD` で指定してください"
    )]
    #[case::malformed_indicator_date(
        "[[indicator:demo-indicator@2030-1-2]]",
        "日付を `YYYY-MM-DD` で指定してください"
    )]
    fn rejects_invalid_price_reference_links(#[case] body: &str, #[case] reason: &str) {
        assert_eq!(
            extract_note_price_references(body),
            Err(vec![NoteTokenValidationError::BodyToken {
                token: body.to_string(),
                reason: reason.into(),
            }]),
        );
    }

    #[test]
    fn ignores_price_reference_links_in_markdown_code() {
        assert_eq!(
            extract_note_price_references(indoc! {"
                `[[price:fictional-code@2030-01-02:close]]`
                `[[valuation:fictional-code@2030-01-02:per]]`
                `[[indicator:demo-indicator@2030-01-02]]`

                ```text
                [[change:fictional-code@2030-01-02..2030-01-03:close]]
                ```
            "}),
            Ok(vec![]),
        );
    }

    #[test]
    fn leaves_date_less_indicator_references_out_of_value_resolution() {
        assert_eq!(
            extract_note_price_references(
                "[[indicator:demo-indicator]] と [[indicator:demo-indicator@2030-01-02]]",
            ),
            Ok(vec![NotePriceReferenceToken {
                token: "[[indicator:demo-indicator@2030-01-02]]".into(),
                reference: NotePriceReference::Indicator {
                    indicator_id: "demo-indicator".into(),
                    date: "2030-01-02".parse().expect("date"),
                },
            }]),
        );
    }
}
