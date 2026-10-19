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
            b'\'' => {
                cursor = skip_single_quoted_string(
                    bytes,
                    cursor,
                    is_escape_string_prefix(bytes, cursor),
                )?;
            }
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

fn is_escape_string_prefix(bytes: &[u8], quote_start: usize) -> bool {
    if quote_start == 0 || !matches!(bytes[quote_start - 1], b'e' | b'E') {
        return false;
    }

    quote_start < 2 || !is_identifier_byte(bytes[quote_start - 2])
}

fn is_identifier_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_' || !byte.is_ascii()
}

fn skip_single_quoted_string(
    bytes: &[u8],
    start: usize,
    escape_backslashes: bool,
) -> Result<usize, String> {
    let mut cursor = start + 1;
    while cursor < bytes.len() {
        match bytes[cursor] {
            b'\\' if escape_backslashes => cursor = (cursor + 2).min(bytes.len()),
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

#[cfg(test)]
mod tests {
    use super::strip_for_update_clause;
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
    #[case::quoted(
        "SELECT 'FOR UPDATE SKIP LOCKED' AS status",
        "SELECT 'FOR UPDATE SKIP LOCKED' AS status"
    )]
    #[case::comment(
        "SELECT id FROM synthetic_records /* FOR UPDATE */",
        "SELECT id FROM synthetic_records /* FOR UPDATE */"
    )]
    #[case::ordinary_backslash("SELECT '\\' AS value FOR UPDATE", "SELECT '\\' AS value ")]
    #[case::escape_string("SELECT E'it\\'s' AS value FOR UPDATE", "SELECT E'it\\'s' AS value ")]
    fn skips_strings_and_comments(#[case] sql: &str, #[case] expected: &str) {
        assert_eq!(strip_for_update_clause(sql), Ok(expected.to_string()));
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
}
