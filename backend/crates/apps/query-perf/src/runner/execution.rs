use std::time::{Duration, Instant};

use chrono::NaiveDate;
use sqlx::PgConnection;

const STATEMENT_TIMEOUT: &str = "5s";

struct SqlToken {
    value: String,
    start: usize,
    end: usize,
    depth: usize,
}

pub(super) fn strip_for_update_clause(sql: &str) -> Result<String, String> {
    let tokens = sql_tokens(sql)?;
    for (index, pair) in tokens.windows(2).enumerate() {
        if pair[0].value != "FOR" || pair[1].value != "UPDATE" {
            continue;
        }
        if pair[0].depth != pair[1].depth || pair[0].depth != 0 {
            return Err("nested FOR UPDATE clauses cannot be replayed read-only".to_string());
        }

        let mut last_clause_token = index + 1;
        if tokens.get(index + 2).map(|token| token.value.as_str()) == Some("SKIP")
            && tokens.get(index + 3).map(|token| token.value.as_str()) == Some("LOCKED")
        {
            last_clause_token = index + 3;
        }
        let next_token = tokens.get(last_clause_token + 1);
        if next_token.is_some_and(|token| token.value != ";")
            || (next_token.is_some_and(|token| token.value == ";")
                && tokens.get(last_clause_token + 2).is_some())
        {
            return Err("unsupported trailing FOR UPDATE clause".to_string());
        }

        return Ok(format!(
            "{}{}",
            &sql[..pair[0].start],
            &sql[tokens[last_clause_token].end..]
        ));
    }

    Ok(sql.to_string())
}

fn sql_tokens(sql: &str) -> Result<Vec<SqlToken>, String> {
    let bytes = sql.as_bytes();
    let mut tokens = Vec::new();
    let mut cursor = 0;
    let mut depth = 0_usize;

    while cursor < bytes.len() {
        match bytes[cursor] {
            byte if byte.is_ascii_whitespace() => cursor += 1,
            b'-' if bytes.get(cursor + 1) == Some(&b'-') => {
                cursor += 2;
                while cursor < bytes.len() && bytes[cursor] != b'\n' {
                    cursor += 1;
                }
            }
            b'/' if bytes.get(cursor + 1) == Some(&b'*') => {
                cursor = skip_block_comment(bytes, cursor)?;
            }
            b'\'' => cursor = skip_single_quoted_string(bytes, cursor)?,
            b'"' => cursor = skip_quoted_identifier(bytes, cursor)?,
            b'$' => {
                if let Some(end) = dollar_quoted_string_end(sql, cursor)? {
                    cursor = end;
                } else {
                    cursor += 1;
                }
            }
            b'(' => {
                depth += 1;
                cursor += 1;
            }
            b')' => {
                depth = depth
                    .checked_sub(1)
                    .ok_or_else(|| "unbalanced closing parenthesis in SQL".to_string())?;
                cursor += 1;
            }
            b';' => {
                if depth == 0 {
                    tokens.push(SqlToken {
                        value: ";".to_string(),
                        start: cursor,
                        end: cursor + 1,
                        depth,
                    });
                }
                cursor += 1;
            }
            byte if byte.is_ascii_alphabetic() || byte == b'_' => {
                let start = cursor;
                cursor += 1;
                while cursor < bytes.len()
                    && (bytes[cursor].is_ascii_alphanumeric() || bytes[cursor] == b'_')
                {
                    cursor += 1;
                }
                tokens.push(SqlToken {
                    value: sql[start..cursor].to_ascii_uppercase(),
                    start,
                    end: cursor,
                    depth,
                });
            }
            _ => cursor += 1,
        }
    }

    if depth != 0 {
        return Err("unbalanced parentheses in SQL".to_string());
    }
    Ok(tokens)
}

fn skip_block_comment(bytes: &[u8], start: usize) -> Result<usize, String> {
    let mut cursor = start + 2;
    let mut depth = 1;
    while cursor < bytes.len() {
        if bytes[cursor] == b'/' && bytes.get(cursor + 1) == Some(&b'*') {
            depth += 1;
            cursor += 2;
        } else if bytes[cursor] == b'*' && bytes.get(cursor + 1) == Some(&b'/') {
            depth -= 1;
            cursor += 2;
            if depth == 0 {
                return Ok(cursor);
            }
        } else {
            cursor += 1;
        }
    }
    Err("unterminated block comment in SQL".to_string())
}

fn skip_single_quoted_string(bytes: &[u8], start: usize) -> Result<usize, String> {
    let mut cursor = start + 1;
    while cursor < bytes.len() {
        match bytes[cursor] {
            b'\\' => cursor = (cursor + 2).min(bytes.len()),
            b'\'' if bytes.get(cursor + 1) == Some(&b'\'') => cursor += 2,
            b'\'' => return Ok(cursor + 1),
            _ => cursor += 1,
        }
    }
    Err("unterminated string literal in SQL".to_string())
}

fn skip_quoted_identifier(bytes: &[u8], start: usize) -> Result<usize, String> {
    let mut cursor = start + 1;
    while cursor < bytes.len() {
        if bytes[cursor] == b'"' && bytes.get(cursor + 1) == Some(&b'"') {
            cursor += 2;
        } else if bytes[cursor] == b'"' {
            return Ok(cursor + 1);
        } else {
            cursor += 1;
        }
    }
    Err("unterminated quoted identifier in SQL".to_string())
}

fn dollar_quoted_string_end(sql: &str, start: usize) -> Result<Option<usize>, String> {
    let bytes = sql.as_bytes();
    let mut delimiter_end = start + 1;
    while delimiter_end < bytes.len()
        && (bytes[delimiter_end].is_ascii_alphanumeric() || bytes[delimiter_end] == b'_')
    {
        delimiter_end += 1;
    }
    if bytes.get(delimiter_end) != Some(&b'$')
        || (delimiter_end > start + 1
            && !bytes[start + 1].is_ascii_alphabetic()
            && bytes[start + 1] != b'_')
    {
        return Ok(None);
    }

    let delimiter = &sql[start..=delimiter_end];
    let content_start = delimiter_end + 1;
    sql[content_start..]
        .find(delimiter)
        .map(|offset| Some(content_start + offset + delimiter.len()))
        .ok_or_else(|| format!("unterminated dollar-quoted string {delimiter}"))
}

#[derive(Debug)]
pub(super) struct RunError {
    pub(super) message: String,
    pub(super) duration: Duration,
    pub(super) fatal: bool,
    pub(super) kind: String,
    pub(super) non_replayable: bool,
}

pub(super) async fn prepare_statement(
    connection: &mut PgConnection,
    statement_name: &str,
    sql: &str,
    parameters: &[Option<String>],
) -> Result<Vec<String>, sqlx::Error> {
    begin_read_only(connection).await?;

    let query = sql.trim().trim_end_matches(';').trim_end();
    let prepare_sql = format!("PREPARE {statement_name} AS {query}");
    let preparation = async {
        sqlx::raw_sql(sqlx::AssertSqlSafe(prepare_sql))
            .execute(&mut *connection)
            .await?;
        let mut parameter_types = prepared_parameter_types(connection, statement_name).await?;
        if resolve_ambiguous_parameter_types(&parameter_types, parameters) {
            let explicit_types = explicit_parameter_types(&parameter_types, parameters)?;
            sqlx::raw_sql(sqlx::AssertSqlSafe(format!("DEALLOCATE {statement_name}")))
                .execute(&mut *connection)
                .await?;
            let prepare_sql = format!(
                "PREPARE {statement_name} ({}) AS {query}",
                explicit_types.join(", ")
            );
            sqlx::raw_sql(sqlx::AssertSqlSafe(prepare_sql))
                .execute(&mut *connection)
                .await?;
            parameter_types = prepared_parameter_types(connection, statement_name).await?;
        }
        Ok(parameter_types)
    }
    .await;

    match preparation {
        Ok(parameter_types) => {
            sqlx::query("COMMIT").execute(&mut *connection).await?;
            Ok(parameter_types)
        }
        Err(error) => match sqlx::query("ROLLBACK").execute(&mut *connection).await {
            Ok(_) => Err(error),
            Err(rollback_error) => Err(rollback_error),
        },
    }
}

pub(super) async fn deallocate_statement(
    connection: &mut PgConnection,
    statement_name: &str,
) -> Result<(), sqlx::Error> {
    sqlx::raw_sql(sqlx::AssertSqlSafe(format!("DEALLOCATE {statement_name}")))
        .execute(&mut *connection)
        .await?;
    Ok(())
}

async fn prepared_parameter_types(
    connection: &mut PgConnection,
    statement_name: &str,
) -> Result<Vec<String>, sqlx::Error> {
    sqlx::query_scalar::<_, String>(
        "SELECT parameter.parameter_type::text \
         FROM pg_prepared_statements AS prepared, \
              LATERAL unnest(prepared.parameter_types) WITH ORDINALITY \
                AS parameter(parameter_type, position) \
         WHERE prepared.name = $1 \
         ORDER BY parameter.position",
    )
    .bind(statement_name)
    .fetch_all(&mut *connection)
    .await
}

fn resolve_ambiguous_parameter_types(
    parameter_types: &[String],
    parameters: &[Option<String>],
) -> bool {
    parameter_types.len() == parameters.len()
        && parameter_types
            .iter()
            .zip(parameters)
            .any(|(type_name, parameter)| {
                type_name == "interval"
                    && parameter.as_deref().and_then(timestamp_type_hint).is_some()
            })
}

fn explicit_parameter_types(
    parameter_types: &[String],
    parameters: &[Option<String>],
) -> Result<Vec<String>, sqlx::Error> {
    parameter_types
        .iter()
        .zip(parameters)
        .map(|(type_name, parameter)| {
            let resolved_type = if type_name == "interval" {
                parameter
                    .as_deref()
                    .and_then(timestamp_type_hint)
                    .unwrap_or(type_name)
            } else {
                type_name
            };
            if !resolved_type
                .chars()
                .all(|character| character.is_ascii_alphanumeric() || "_ .[]".contains(character))
            {
                return Err(sqlx::Error::Protocol(format!(
                    "cannot safely render PostgreSQL parameter type {resolved_type:?}"
                )));
            }
            Ok(resolved_type.to_string())
        })
        .collect()
}

fn timestamp_type_hint(value: &str) -> Option<&'static str> {
    let date_text = value.get(..10)?;
    NaiveDate::parse_from_str(date_text, "%Y-%m-%d").ok()?;
    let bytes = value.as_bytes();
    if !matches!(bytes.get(10), Some(b' ' | b'T')) {
        return None;
    }
    let time = value.get(11..19)?;
    if time.len() != 8
        || !time
            .as_bytes()
            .iter()
            .enumerate()
            .all(|(index, byte)| match index {
                2 | 5 => *byte == b':',
                _ => byte.is_ascii_digit(),
            })
    {
        return None;
    }
    let suffix = value.get(19..)?;
    if !suffix.is_empty()
        && !suffix.starts_with('.')
        && !suffix.starts_with('+')
        && !suffix.starts_with('-')
        && !suffix.eq_ignore_ascii_case("z")
    {
        return None;
    }

    if suffix.contains('+') || suffix.contains('-') || suffix.eq_ignore_ascii_case("z") {
        Some("timestamp with time zone")
    } else {
        Some("timestamp without time zone")
    }
}

async fn begin_read_only(connection: &mut PgConnection) -> Result<(), sqlx::Error> {
    sqlx::query("BEGIN READ ONLY")
        .execute(&mut *connection)
        .await?;
    let timeout_statement = format!("SET LOCAL statement_timeout = '{STATEMENT_TIMEOUT}'");
    if let Err(error) = sqlx::raw_sql(sqlx::AssertSqlSafe(timeout_statement))
        .execute(&mut *connection)
        .await
    {
        match sqlx::query("ROLLBACK").execute(&mut *connection).await {
            Ok(_) => Err(error),
            Err(rollback_error) => Err(rollback_error),
        }
    } else {
        Ok(())
    }
}

pub(super) async fn execute_read_only(
    connection: &mut PgConnection,
    execute_sql: &str,
) -> Result<Duration, RunError> {
    if let Err(error) = begin_read_only(connection).await {
        return Err(RunError {
            message: error.to_string(),
            duration: Duration::ZERO,
            fatal: !is_database_error(&error),
            kind: error_kind(&error),
            non_replayable: is_non_replayable_error(&error),
        });
    }

    let started = Instant::now();
    let execution = sqlx::raw_sql(sqlx::AssertSqlSafe(execute_sql.to_string()))
        .execute(&mut *connection)
        .await;
    let duration = started.elapsed();
    let rollback = sqlx::query("ROLLBACK").execute(&mut *connection).await;

    match (execution, rollback) {
        (Ok(_), Ok(_)) => Ok(duration),
        (Err(error), Ok(_)) => Err(RunError {
            message: error.to_string(),
            duration,
            fatal: !is_database_error(&error),
            kind: error_kind(&error),
            non_replayable: is_non_replayable_error(&error),
        }),
        (Ok(_), Err(error)) => Err(RunError {
            message: format!("query completed but rollback failed: {error}"),
            duration,
            fatal: true,
            kind: error_kind(&error),
            non_replayable: false,
        }),
        (Err(error), Err(rollback_error)) => Err(RunError {
            message: format!("{error}; rollback failed: {rollback_error}"),
            duration,
            fatal: true,
            kind: error_kind(&error),
            non_replayable: false,
        }),
    }
}

pub(super) fn error_kind(error: &sqlx::Error) -> String {
    if error
        .as_database_error()
        .and_then(sqlx::error::DatabaseError::code)
        .is_some_and(|code| code == "57014")
    {
        "statement timeout".to_string()
    } else if is_database_error(error) {
        "execution error".to_string()
    } else {
        "connection error (possible OOM disconnect)".to_string()
    }
}

pub(super) fn is_database_error(error: &sqlx::Error) -> bool {
    error.as_database_error().is_some()
}

pub(super) fn is_non_replayable_error(error: &sqlx::Error) -> bool {
    error
        .as_database_error()
        .and_then(sqlx::error::DatabaseError::code)
        .is_some_and(|code| is_non_replayable_sqlstate(code.as_ref()))
}

fn is_non_replayable_sqlstate(code: &str) -> bool {
    matches!(code, "25006" | "42P01" | "42703")
}

#[cfg(test)]
mod tests {
    use super::{
        explicit_parameter_types, is_non_replayable_sqlstate, resolve_ambiguous_parameter_types,
        strip_for_update_clause, timestamp_type_hint,
    };
    use rstest::rstest;

    #[rstest]
    #[case::update(
        "SELECT id FROM synthetic_records FOR UPDATE",
        "SELECT id FROM synthetic_records "
    )]
    #[case::skip_locked(
        "SELECT id FROM synthetic_records FOR UPDATE SKIP LOCKED;",
        "SELECT id FROM synthetic_records ;"
    )]
    fn strips_trailing_for_update_clause(#[case] sql: &str, #[case] expected: &str) {
        assert_eq!(strip_for_update_clause(sql), Ok(expected.to_string()));
    }

    #[rstest]
    #[case::quoted("SELECT 'FOR UPDATE SKIP LOCKED' AS status")]
    #[case::comment("SELECT id FROM synthetic_records /* FOR UPDATE */")]
    fn leaves_lock_words_in_strings_and_comments_unchanged(#[case] sql: &str) {
        assert_eq!(strip_for_update_clause(sql), Ok(sql.to_string()));
    }

    #[rstest]
    #[case::nested(
        "SELECT * FROM (SELECT id FROM synthetic_records FOR UPDATE) AS nested",
        "nested FOR UPDATE clauses cannot be replayed read-only"
    )]
    #[case::nowait(
        "SELECT id FROM synthetic_records FOR UPDATE NOWAIT",
        "unsupported trailing FOR UPDATE clause"
    )]
    fn rejects_unsupported_locking_clauses(#[case] sql: &str, #[case] expected_reason: &str) {
        assert_eq!(
            strip_for_update_clause(sql),
            Err(expected_reason.to_string())
        );
    }

    #[test]
    fn resolves_timestamp_values_that_postgres_inferred_as_intervals() {
        let parameter_types = vec!["interval".to_string()];
        let parameters = vec![Some("2026-10-18 20:27:19.720876+00".to_string())];

        assert_eq!(
            (
                resolve_ambiguous_parameter_types(&parameter_types, &parameters),
                explicit_parameter_types(&parameter_types, &parameters)
                    .map_err(|error| error.to_string()),
                timestamp_type_hint("2026-10-18 20:27:19.720876+00"),
            ),
            (
                true,
                Ok(vec!["timestamp with time zone".to_string()]),
                Some("timestamp with time zone"),
            )
        );
    }
    #[rstest]
    #[case::read_only_transaction("25006", true)]
    #[case::missing_relation("42P01", true)]
    #[case::missing_column("42703", true)]
    #[case::statement_timeout("57014", false)]
    #[case::invalid_input("22007", false)]
    fn allows_only_known_non_replayable_sqlstates(#[case] code: &str, #[case] expected: bool) {
        assert_eq!(is_non_replayable_sqlstate(code), expected);
    }
}
