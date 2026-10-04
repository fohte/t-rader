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
        .filter(|token| is_price_reference_kind(token.inner))
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

fn is_price_reference_kind(token: &str) -> bool {
    matches!(
        token.split_once(':').map_or(token, |(kind, _)| kind),
        "price" | "change"
    )
}

fn parse_note_price_reference(token: &str) -> Result<NotePriceReference, &'static str> {
    let (kind, content) = token
        .split_once(':')
        .ok_or("`price:<id>@<date>:<field>` または `change:<id>@<start>..<end>:<field>` を指定してください")?;
    let (instrument_and_dates, field) = content
        .rsplit_once(':')
        .ok_or("項目を `open`, `high`, `low`, `close`, `volume` のいずれかで指定してください")?;
    let field = parse_field(field)
        .ok_or("項目を `open`, `high`, `low`, `close`, `volume` のいずれかで指定してください")?;
    let (instrument_id, dates) = instrument_and_dates
        .rsplit_once('@')
        .ok_or("銘柄 ID と日付を `@` で区切ってください")?;
    if !is_valid_instrument_id(instrument_id) {
        return Err("銘柄 ID を空にせず、空白や `@` を含めないでください");
    }

    match kind {
        "price" => Ok(NotePriceReference::Price {
            instrument_id: instrument_id.to_string(),
            date: parse_date(dates)?,
            field,
        }),
        "change" => {
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
        _ => Err("価格参照は `price` または `change` を指定してください"),
    }
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

fn parse_field(value: &str) -> Option<PriceReferenceField> {
    match value {
        "open" => Some(PriceReferenceField::Open),
        "high" => Some(PriceReferenceField::High),
        "low" => Some(PriceReferenceField::Low),
        "close" => Some(PriceReferenceField::Close),
        "volume" => Some(PriceReferenceField::Volume),
        _ => None,
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
        NotePriceReference, NotePriceReferenceToken, PriceReferenceField,
        extract_note_price_references,
    };
    use crate::note_reference::NoteTokenValidationError;

    #[rstest]
    #[case::price_close("[[price:fictional-code@2030-01-02:close]]", NotePriceReference::Price { instrument_id: "fictional-code".into(), date: "2030-01-02".parse().expect("date"), field: PriceReferenceField::Close })]
    #[case::foreign_stock_id("[[price:US:FICTIONAL-A@2030-01-02:open]]", NotePriceReference::Price { instrument_id: "US:FICTIONAL-A".into(), date: "2030-01-02".parse().expect("date"), field: PriceReferenceField::Open })]
    #[case::price_high("[[price:fictional-code@2030-01-02:high]]", NotePriceReference::Price { instrument_id: "fictional-code".into(), date: "2030-01-02".parse().expect("date"), field: PriceReferenceField::High })]
    #[case::price_low("[[price:fictional-code@2030-01-02:low]]", NotePriceReference::Price { instrument_id: "fictional-code".into(), date: "2030-01-02".parse().expect("date"), field: PriceReferenceField::Low })]
    #[case::change_volume("[[change:fictional-code@2030-01-02..2030-01-03:volume]]", NotePriceReference::Change { instrument_id: "fictional-code".into(), start: "2030-01-02".parse().expect("start date"), end: "2030-01-03".parse().expect("end date"), field: PriceReferenceField::Volume })]
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

                ```text
                [[change:fictional-code@2030-01-02..2030-01-03:close]]
                ```
            "}),
            Ok(vec![]),
        );
    }
}
