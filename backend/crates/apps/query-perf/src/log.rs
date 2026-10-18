use std::collections::{BTreeMap, BTreeSet};

use serde::Deserialize;

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd)]
pub struct Exclusion {
    pub sql: String,
    pub reason: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ParsedLog {
    pub statements: BTreeMap<String, BTreeSet<Vec<Option<String>>>>,
    pub excluded: Vec<ExcludedStatement>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExcludedStatement {
    pub sql: String,
    pub reason: String,
}

pub fn parse_json_log(contents: &str, exclusions: &[Exclusion]) -> Result<ParsedLog, String> {
    let mut parsed = ParsedLog::default();
    let mut excluded = BTreeSet::new();

    for (line_index, line) in contents.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }

        let entry: serde_json::Value = serde_json::from_str(line)
            .map_err(|error| format!("JSON log line {}: {error}", line_index + 1))?;
        let Some((sql, detail)) = query_from_entry(&entry)
            .map_err(|error| format!("JSON log line {}: {error}", line_index + 1))?
        else {
            continue;
        };
        if !is_select_or_with(&sql) {
            continue;
        }

        match parse_parameters(detail.as_deref()) {
            Ok(parameters) => {
                parsed.statements.entry(sql).or_default().insert(parameters);
            }
            Err(reason) => {
                if let Some(exclusion) = exclusions.iter().find(|entry| entry.sql == sql) {
                    if excluded.insert(sql.clone()) {
                        parsed.excluded.push(ExcludedStatement {
                            sql,
                            reason: exclusion.reason.clone(),
                        });
                    }
                } else {
                    return Err(format!(
                        "JSON log line {}: cannot parse parameters for SQL `{sql}`: {reason}",
                        line_index + 1
                    ));
                }
            }
        }
    }

    Ok(parsed)
}

fn query_from_entry(entry: &serde_json::Value) -> Result<Option<(String, Option<String>)>, String> {
    let message = entry.get("message").and_then(serde_json::Value::as_str);
    let statement = entry.get("statement").and_then(serde_json::Value::as_str);

    let sql = if let Some(message) = message {
        if let Some(execute) = message.strip_prefix("execute ") {
            let (_, sql) = execute
                .split_once(": ")
                .ok_or_else(|| format!("unrecognized Execute log message: {message}"))?;
            Some(sql.trim().to_string())
        } else if let Some(sql) = message.strip_prefix("statement: ") {
            Some(sql.trim().to_string())
        } else {
            statement.map(str::trim).map(str::to_string)
        }
    } else {
        statement.map(str::trim).map(str::to_string)
    };

    let Some(sql) = sql else {
        return Ok(None);
    };
    let detail = entry
        .get("detail")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string);

    Ok(Some((sql, detail)))
}

fn is_select_or_with(sql: &str) -> bool {
    let mut rest = sql.trim_start();
    loop {
        if let Some(comment) = rest.strip_prefix("--") {
            rest = comment
                .find('\n')
                .map(|newline| &comment[newline + 1..])
                .unwrap_or("")
                .trim_start();
            continue;
        }
        if let Some(comment) = rest.strip_prefix("/*") {
            let Some(end) = comment.find("*/") else {
                return false;
            };
            rest = comment[end + 2..].trim_start();
            continue;
        }

        let keyword: String = rest
            .chars()
            .take_while(|character| character.is_ascii_alphabetic())
            .collect();
        return keyword.eq_ignore_ascii_case("SELECT") || keyword.eq_ignore_ascii_case("WITH");
    }
}

fn parse_parameters(detail: Option<&str>) -> Result<Vec<Option<String>>, String> {
    let Some(detail) = detail else {
        return Ok(Vec::new());
    };
    let Some((kind, mut rest)) = detail.trim().split_once(':') else {
        return Err(format!("unexpected query detail: {detail}"));
    };
    if !kind.trim().eq_ignore_ascii_case("parameters") {
        return Err(format!("unexpected query detail: {detail}"));
    }
    rest = rest.trim_start();
    if rest.is_empty() {
        return Err("parameter detail is empty".to_string());
    }

    let bytes = rest.as_bytes();
    let mut values = BTreeMap::new();
    let mut cursor = 0;
    while cursor < bytes.len() {
        while cursor < bytes.len() && (bytes[cursor].is_ascii_whitespace() || bytes[cursor] == b',')
        {
            cursor += 1;
        }
        if cursor == bytes.len() {
            break;
        }
        if bytes[cursor] != b'$' {
            return Err(format!(
                "expected parameter index near `{}`",
                &rest[cursor..]
            ));
        }
        cursor += 1;
        let index_start = cursor;
        while cursor < bytes.len() && bytes[cursor].is_ascii_digit() {
            cursor += 1;
        }
        if index_start == cursor {
            return Err(format!(
                "missing parameter index near `{}`",
                &rest[index_start..]
            ));
        }
        let index = rest[index_start..cursor]
            .parse::<usize>()
            .map_err(|error| format!("invalid parameter index: {error}"))?;
        while cursor < bytes.len() && bytes[cursor].is_ascii_whitespace() {
            cursor += 1;
        }
        if bytes.get(cursor) != Some(&b'=') {
            return Err(format!("missing `=` for parameter ${index}"));
        }
        cursor += 1;
        while cursor < bytes.len() && bytes[cursor].is_ascii_whitespace() {
            cursor += 1;
        }

        let value_start = cursor;
        let mut in_quote = false;
        while cursor < bytes.len() {
            if bytes[cursor] == b'\'' {
                if in_quote && bytes.get(cursor + 1) == Some(&b'\'') {
                    cursor += 2;
                    continue;
                }
                in_quote = !in_quote;
                cursor += 1;
                continue;
            }
            if bytes[cursor] == b',' && !in_quote && next_parameter_start(bytes, cursor + 1) {
                break;
            }
            cursor += 1;
        }
        if in_quote {
            return Err(format!("unterminated quoted value for parameter ${index}"));
        }
        let raw_value = rest[value_start..cursor].trim();
        if raw_value.is_empty() {
            return Err(format!("empty value for parameter ${index}"));
        }
        if values.insert(index, decode_parameter(raw_value)?).is_some() {
            return Err(format!("duplicate parameter ${index}"));
        }
    }

    let Some(max_index) = values.keys().next_back().copied() else {
        return Ok(Vec::new());
    };
    (1..=max_index)
        .map(|index| {
            values
                .remove(&index)
                .ok_or_else(|| format!("missing parameter ${index}"))
        })
        .collect()
}

fn next_parameter_start(bytes: &[u8], mut cursor: usize) -> bool {
    while cursor < bytes.len() && bytes[cursor].is_ascii_whitespace() {
        cursor += 1;
    }
    if bytes.get(cursor) != Some(&b'$') {
        return false;
    }
    cursor += 1;
    let index_start = cursor;
    while cursor < bytes.len() && bytes[cursor].is_ascii_digit() {
        cursor += 1;
    }
    if cursor == index_start {
        return false;
    }
    while cursor < bytes.len() && bytes[cursor].is_ascii_whitespace() {
        cursor += 1;
    }
    bytes.get(cursor) == Some(&b'=')
}

fn decode_parameter(raw: &str) -> Result<Option<String>, String> {
    if raw.eq_ignore_ascii_case("NULL") {
        return Ok(None);
    }
    if !raw.starts_with('\'') {
        return Ok(Some(raw.to_string()));
    }
    if !raw.ends_with('\'') || raw.len() < 2 {
        return Err(format!("invalid quoted parameter value: {raw}"));
    }

    let mut value = String::new();
    let mut characters = raw[1..raw.len() - 1].chars().peekable();
    while let Some(character) = characters.next() {
        if character == '\'' && characters.next_if_eq(&'\'').is_none() {
            return Err(format!("invalid quote escape in parameter value: {raw}"));
        }
        value.push(character);
    }

    Ok(Some(value))
}

#[cfg(test)]
mod tests {
    use super::{Exclusion, ParsedLog, parse_json_log};
    use indoc::indoc;
    use std::collections::{BTreeMap, BTreeSet};

    #[test]
    fn parses_execute_sql_and_deduplicates_identical_parameters() {
        let logs = indoc! {r#"
            {"message":"execute <unnamed>: SELECT id FROM synthetic_records WHERE created_at >= $1","detail":"parameters: $1 = '2021-03-04'"}
            {"message":"execute <unnamed>: SELECT id FROM synthetic_records WHERE created_at >= $1","detail":"Parameters: $1 = '2021-03-04'"}
            {"message":"execute <unnamed>: INSERT INTO synthetic_records VALUES ($1)","detail":"parameters: $1 = 42"}
            {"message":"connection authorized"}
        "#};

        assert_eq!(
            parse_json_log(logs, &[]),
            Ok(ParsedLog {
                statements: BTreeMap::from([(
                    "SELECT id FROM synthetic_records WHERE created_at >= $1".to_string(),
                    BTreeSet::from([vec![Some("2021-03-04".to_string())]])
                )]),
                excluded: Vec::new(),
            })
        );
    }

    #[test]
    fn parses_quoted_commas_null_and_doubled_quotes() {
        let logs = "{\"message\":\"execute <unnamed>: WITH records AS (SELECT $1) SELECT * FROM records\",\"detail\":\"parameters: $1 = 'label, $2 = ''quoted''', $2 = NULL\"}";

        assert_eq!(
            parse_json_log(logs, &[]).map(|parsed| parsed.statements),
            Ok(BTreeMap::from([(
                "WITH records AS (SELECT $1) SELECT * FROM records".to_string(),
                BTreeSet::from([vec![Some("label, $2 = 'quoted'".to_string()), None,]])
            )]))
        );
    }

    #[test]
    fn skips_a_parameter_parse_failure_only_when_the_sql_is_excluded() {
        let logs = "{\"message\":\"execute <unnamed>: SELECT * FROM synthetic_records WHERE id = $1\",\"detail\":\"parameters: invalid\"}";
        let exclusions = vec![Exclusion {
            sql: "SELECT * FROM synthetic_records WHERE id = $1".to_string(),
            reason: "The driver emits an unsupported parameter format.".to_string(),
        }];

        assert_eq!(
            parse_json_log(logs, &exclusions),
            Ok(ParsedLog {
                statements: BTreeMap::new(),
                excluded: vec![super::ExcludedStatement {
                    sql: "SELECT * FROM synthetic_records WHERE id = $1".to_string(),
                    reason: "The driver emits an unsupported parameter format.".to_string(),
                }],
            })
        );
    }
}
