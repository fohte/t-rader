use chrono::{Duration as ChronoDuration, NaiveDate};

pub(super) fn shift_parameters(
    parameters: &[Option<String>],
    parameter_types: &[String],
    target_date: NaiveDate,
) -> Result<Vec<Option<String>>, String> {
    if parameters.len() != parameter_types.len() {
        return Err(format!(
            "log contains {} parameter(s), while PostgreSQL inferred {} parameter type(s)",
            parameters.len(),
            parameter_types.len()
        ));
    }

    let kinds: Vec<_> = parameter_types
        .iter()
        .map(|type_name| date_parameter_kind(type_name))
        .collect();
    let mut latest_typed_date: Option<NaiveDate> = None;
    let mut latest_text_date: Option<NaiveDate> = None;
    for (parameter, kind) in parameters.iter().zip(&kinds) {
        let (Some(value), Some(kind)) = (parameter, kind) else {
            continue;
        };
        let latest_date = if matches!(*kind, DateParameterKind::Text) {
            &mut latest_text_date
        } else {
            &mut latest_typed_date
        };
        for date in parameter_dates(value, *kind)? {
            *latest_date = Some(match *latest_date {
                Some(current) => current.max(date),
                None => date,
            });
        }
    }
    let day_shift = latest_typed_date
        .or(latest_text_date)
        .map(|date| target_date.signed_duration_since(date).num_days())
        .unwrap_or(0);

    parameters
        .iter()
        .zip(kinds)
        .map(|(parameter, kind)| match (parameter, kind) {
            (Some(value), Some(kind)) => shift_parameter_value(value, kind, day_shift).map(Some),
            (value, _) => Ok(value.clone()),
        })
        .collect()
}

#[derive(Clone, Copy)]
enum DateParameterKind {
    Date,
    Timestamp,
    Array,
    Text,
}

fn date_parameter_kind(type_name: &str) -> Option<DateParameterKind> {
    let normalized = type_name.trim().to_ascii_lowercase();
    let is_array = normalized.ends_with("[]") || normalized.starts_with('_');
    let base = if let Some(base) = normalized.strip_suffix("[]") {
        base
    } else if is_array {
        normalized.strip_prefix('_').unwrap_or(&normalized)
    } else {
        &normalized
    };
    let kind = match base {
        "date" | "pg_catalog.date" => DateParameterKind::Date,
        "timestamp"
        | "timestamp without time zone"
        | "timestamptz"
        | "timestamp with time zone"
        | "pg_catalog.timestamp"
        | "pg_catalog.timestamptz" => DateParameterKind::Timestamp,
        "text" | "pg_catalog.text" | "varchar" | "pg_catalog.varchar" | "character varying" => {
            DateParameterKind::Text
        }
        _ => return None,
    };
    match (is_array, kind) {
        (true, DateParameterKind::Text) => None,
        (true, _) => Some(DateParameterKind::Array),
        (false, kind) => Some(kind),
    }
}

fn parameter_dates(value: &str, kind: DateParameterKind) -> Result<Vec<NaiveDate>, String> {
    match kind {
        DateParameterKind::Date => Ok(vec![parse_date_prefix(value, true)?]),
        DateParameterKind::Timestamp => Ok(vec![parse_date_prefix(value, false)?]),
        DateParameterKind::Array => Ok(iso_dates(value)),
        DateParameterKind::Text => Ok(parse_iso_date(value).into_iter().collect()),
    }
}

fn parse_date_prefix(value: &str, date_only: bool) -> Result<NaiveDate, String> {
    let date_text = value
        .get(..10)
        .ok_or_else(|| format!("date parameter is shorter than 10 characters: {value:?}"))?;
    let date = NaiveDate::parse_from_str(date_text, "%Y-%m-%d")
        .map_err(|error| format!("cannot parse date parameter {value:?}: {error}"))?;
    let suffix = value
        .get(10..)
        .ok_or_else(|| format!("invalid UTF-8 boundary in date parameter {value:?}"))?;
    if date_only && !suffix.is_empty() {
        return Err(format!("expected a date parameter, got {value:?}"));
    }
    Ok(date)
}

fn iso_dates(value: &str) -> Vec<NaiveDate> {
    let bytes = value.as_bytes();
    if bytes.len() < 10 {
        return Vec::new();
    }
    (0..=bytes.len() - 10)
        .filter_map(|start| iso_date_at(value, start))
        .collect()
}

fn iso_date_at(value: &str, start: usize) -> Option<NaiveDate> {
    let candidate = value.get(start..start + 10)?;
    if !candidate
        .as_bytes()
        .iter()
        .enumerate()
        .all(|(index, byte)| match index {
            4 | 7 => *byte == b'-',
            _ => byte.is_ascii_digit(),
        })
    {
        return None;
    }
    NaiveDate::parse_from_str(candidate, "%Y-%m-%d").ok()
}

fn shift_parameter_value(
    value: &str,
    kind: DateParameterKind,
    day_shift: i64,
) -> Result<String, String> {
    match kind {
        DateParameterKind::Date => shift_date_prefix(value, day_shift, true),
        DateParameterKind::Timestamp => shift_date_prefix(value, day_shift, false),
        DateParameterKind::Array => shift_date_array(value, day_shift),
        DateParameterKind::Text if parse_iso_date(value).is_some() => {
            shift_date_prefix(value, day_shift, true)
        }
        DateParameterKind::Text => Ok(value.to_string()),
    }
}

fn parse_iso_date(value: &str) -> Option<NaiveDate> {
    (value.len() == 10)
        .then(|| NaiveDate::parse_from_str(value, "%Y-%m-%d").ok())
        .flatten()
}

fn shift_date_prefix(value: &str, day_shift: i64, date_only: bool) -> Result<String, String> {
    let date = parse_date_prefix(value, date_only)?;
    let shifted = date
        .checked_add_signed(ChronoDuration::days(day_shift))
        .ok_or_else(|| format!("date shift is out of range for {value:?}"))?;
    let suffix = value
        .get(10..)
        .ok_or_else(|| format!("invalid UTF-8 boundary in date parameter {value:?}"))?;
    Ok(format!("{}{}", shifted.format("%Y-%m-%d"), suffix))
}

fn shift_date_array(value: &str, day_shift: i64) -> Result<String, String> {
    let bytes = value.as_bytes();
    if bytes.len() < 10 {
        return Ok(value.to_string());
    }

    let mut output = String::with_capacity(value.len());
    let mut copied_until = 0;
    for start in 0..=bytes.len() - 10 {
        let Some(date) = iso_date_at(value, start) else {
            continue;
        };
        let shifted = date
            .checked_add_signed(ChronoDuration::days(day_shift))
            .ok_or_else(|| format!("date shift is out of range for {value:?}"))?;
        output.push_str(
            value.get(copied_until..start).ok_or_else(|| {
                format!("invalid UTF-8 boundary in date array parameter {value:?}")
            })?,
        );
        output.push_str(&shifted.format("%Y-%m-%d").to_string());
        copied_until = start + 10;
    }
    output.push_str(
        value
            .get(copied_until..)
            .ok_or_else(|| format!("invalid UTF-8 boundary in date array parameter {value:?}"))?,
    );
    Ok(output)
}

pub(super) fn build_execute_sql(
    statement_name: &str,
    parameters: &[Option<String>],
) -> Result<String, String> {
    if parameters.is_empty() {
        return Ok(format!("EXECUTE {statement_name}"));
    }

    let values = parameters
        .iter()
        .map(|parameter| match parameter {
            None => Ok("NULL".to_string()),
            Some(value) => dollar_quote(value),
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(format!("EXECUTE {statement_name}({})", values.join(", ")))
}

fn dollar_quote(value: &str) -> Result<String, String> {
    if value.contains('\0') {
        return Err("PostgreSQL text literals cannot contain a NUL byte".to_string());
    }

    let mut index = 0;
    loop {
        let tag = if index == 0 {
            "$query_perf$".to_string()
        } else {
            format!("$query_perf_{index}$")
        };
        if !value.contains(&tag) {
            return Ok(format!("{tag}{value}{tag}"));
        }
        index += 1;
    }
}

pub(super) fn format_parameters(parameters: &[Option<String>]) -> String {
    format!("{parameters:?}")
}

#[cfg(test)]
mod tests {
    use super::{
        DateParameterKind, build_execute_sql, date_parameter_kind, shift_date_prefix,
        shift_parameters,
    };
    use chrono::NaiveDate;
    use rstest::rstest;

    #[rstest]
    #[case::date("date", Some("2020-02-03"), "2026-10-16")]
    #[case::timestamp(
        "timestamp without time zone",
        Some("2020-02-03 09:30:00.123"),
        "2026-10-16 09:30:00.123"
    )]
    #[case::timestamptz(
        "timestamp with time zone",
        Some("2020-02-03 09:30:00+00"),
        "2026-10-16 09:30:00+00"
    )]
    fn shifts_typed_dates_to_the_perf_database_end_date(
        #[case] type_name: &str,
        #[case] value: Option<&str>,
        #[case] expected: &str,
    ) -> Result<(), std::io::Error> {
        let target_date =
            NaiveDate::parse_from_str("2026-10-16", "%Y-%m-%d").map_err(std::io::Error::other)?;
        let shifted = shift_parameters(
            &[value.map(str::to_string)],
            &[type_name.to_string()],
            target_date,
        )
        .map_err(std::io::Error::other)?;

        assert_eq!(shifted, vec![Some(expected.to_string())]);
        Ok(())
    }

    #[test]
    fn shifts_every_date_in_a_date_array_by_one_shared_day_offset() -> Result<(), std::io::Error> {
        let shifted = shift_parameters(
            &[Some("{2020-02-02,2020-02-03}".to_string())],
            &["date[]".to_string()],
            NaiveDate::parse_from_str("2026-10-16", "%Y-%m-%d").map_err(std::io::Error::other)?,
        )
        .map_err(std::io::Error::other)?;

        assert_eq!(shifted, vec![Some("{2026-10-15,2026-10-16}".to_string())]);
        Ok(())
    }

    #[rstest]
    #[case::text("text")]
    #[case::varchar("varchar")]
    #[case::character_varying("character varying")]
    fn shifts_iso_date_text_and_keeps_other_text_unchanged(
        #[case] date_type: &str,
    ) -> Result<(), std::io::Error> {
        let shifted = shift_parameters(
            &[
                Some("2021-01-03".to_string()),
                None,
                Some("record-7".to_string()),
            ],
            &[
                date_type.to_string(),
                "text".to_string(),
                "text".to_string(),
            ],
            NaiveDate::parse_from_str("2026-10-16", "%Y-%m-%d").map_err(std::io::Error::other)?,
        )
        .map_err(std::io::Error::other)?;

        assert_eq!(
            shifted,
            vec![
                Some("2026-10-16".to_string()),
                None,
                Some("record-7".to_string()),
            ]
        );
        Ok(())
    }

    #[rstest]
    #[case::timestamp_array("timestamp with time zone[]", Some("array"))]
    #[case::text("text", Some("text"))]
    #[case::varchar("varchar", Some("text"))]
    #[case::character_varying("character varying", Some("text"))]
    #[case::date("date", Some("date"))]
    #[case::timestamp("timestamp with time zone", Some("timestamp"))]
    #[case::text_array("text[]", None)]
    #[case::pg_catalog_text_array("pg_catalog.text[]", None)]
    #[case::catalog_text_array("_text", None)]
    #[case::varchar_array("varchar[]", None)]
    #[case::character_varying_array("character varying[]", None)]
    fn recognizes_postgres_date_type_names(
        #[case] type_name: &str,
        #[case] expected: Option<&str>,
    ) {
        assert_eq!(
            date_parameter_kind(type_name).map(|kind| match kind {
                DateParameterKind::Date => "date",
                DateParameterKind::Timestamp => "timestamp",
                DateParameterKind::Array => "array",
                DateParameterKind::Text => "text",
            }),
            expected
        );
    }

    #[rstest]
    #[case::text_array("text[]")]
    #[case::varchar_array("varchar[]")]
    #[case::character_varying_array("character varying[]")]
    fn keeps_iso_dates_in_text_arrays_unchanged(
        #[case] type_name: &str,
    ) -> Result<(), std::io::Error> {
        let shifted = shift_parameters(
            &[Some("{2020-02-03,2020-02-04}".to_string())],
            &[type_name.to_string()],
            NaiveDate::parse_from_str("2026-10-16", "%Y-%m-%d").map_err(std::io::Error::other)?,
        )
        .map_err(std::io::Error::other)?;

        assert_eq!(shifted, vec![Some("{2020-02-03,2020-02-04}".to_string())]);
        Ok(())
    }

    #[rstest]
    #[case::date("date", "2020-02-03", "2026-10-16")]
    #[case::timestamp(
        "timestamp with time zone",
        "2020-02-03 09:30:00+00",
        "2026-10-16 09:30:00+00"
    )]
    fn text_date_does_not_change_the_shift_anchor_when_a_typed_date_exists(
        #[case] typed_type: &str,
        #[case] typed_value: &str,
        #[case] expected_typed_value: &str,
    ) -> Result<(), std::io::Error> {
        let shifted = shift_parameters(
            &[
                Some(typed_value.to_string()),
                Some("2020-02-04".to_string()),
            ],
            &[typed_type.to_string(), "text".to_string()],
            NaiveDate::parse_from_str("2026-10-16", "%Y-%m-%d").map_err(std::io::Error::other)?,
        )
        .map_err(std::io::Error::other)?;

        assert_eq!(
            shifted,
            vec![
                Some(expected_typed_value.to_string()),
                Some("2026-10-17".to_string()),
            ]
        );
        Ok(())
    }

    #[test]
    fn shifts_a_timestamp_value_without_a_time_part() {
        assert_eq!(
            shift_date_prefix("2020-02-03", 1, false),
            Ok("2020-02-04".to_string())
        );
    }

    #[test]
    fn uses_an_unused_dollar_quote_tag_when_the_value_contains_the_default_tag() {
        assert_eq!(
            build_execute_sql(
                "query_perf_0",
                &[Some("payload $query_perf$ delimiter".to_string())]
            ),
            Ok(
                "EXECUTE query_perf_0($query_perf_1$payload $query_perf$ delimiter$query_perf_1$)"
                    .to_string()
            )
        );
    }
}
