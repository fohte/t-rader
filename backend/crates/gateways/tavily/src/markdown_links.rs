use reqwest::Url;

pub(crate) fn absolutize_markdown_links(snippet: &str, page_url: &str) -> String {
    let Ok(base_url) = Url::parse(page_url) else {
        return snippet.to_owned();
    };

    let mut result = String::with_capacity(snippet.len());
    let mut copied_until = 0;
    let mut search_from = 0;

    while let Some((target_start, target_end, link_end)) =
        next_markdown_link_target(snippet, search_from)
    {
        search_from = link_end;
        let target = &snippet[target_start..target_end];
        let has_angle_brackets = target.starts_with('<') && target.ends_with('>');
        let destination = if has_angle_brackets {
            &target[1..target.len() - 1]
        } else {
            target
        };
        if Url::parse(destination).is_ok() {
            continue;
        }

        let unescaped_target = unescape_markdown_destination(destination);
        let Ok(absolute_url) = base_url.join(&unescaped_target) else {
            continue;
        };

        result.push_str(&snippet[copied_until..target_start]);
        if has_angle_brackets {
            result.push('<');
        }
        result.push_str(absolute_url.as_str());
        if has_angle_brackets {
            result.push('>');
        }
        copied_until = target_end;
    }

    result.push_str(&snippet[copied_until..]);
    result
}

fn next_markdown_link_target(
    snippet: &str,
    mut search_from: usize,
) -> Option<(usize, usize, usize)> {
    let bytes = snippet.as_bytes();
    while search_from + 1 < bytes.len() {
        if bytes[search_from] == b'`' && !is_escaped(bytes, search_from) {
            let tick_count = count_run(bytes, search_from, b'`');
            if let Some(code_end) = find_code_span_end(bytes, search_from, tick_count) {
                search_from = code_end;
                continue;
            }
            search_from += tick_count;
            continue;
        }

        if bytes[search_from] == b']'
            && bytes[search_from + 1] == b'('
            && !is_escaped(bytes, search_from)
            && let Some((target_start, target_end, link_end)) =
                parse_markdown_link_target(bytes, search_from + 1)
        {
            return Some((target_start, target_end, link_end));
        }

        search_from += 1;
    }
    None
}

fn parse_markdown_link_target(bytes: &[u8], open_paren: usize) -> Option<(usize, usize, usize)> {
    let mut cursor = skip_whitespace(bytes, open_paren + 1);
    let (target_start, target_end) = if bytes.get(cursor) == Some(&b'<') {
        let target_start = cursor;
        cursor += 1;
        while cursor < bytes.len() && (bytes[cursor] != b'>' || is_escaped(bytes, cursor)) {
            cursor += 1;
        }
        if cursor == bytes.len() {
            return None;
        }
        cursor += 1;
        let target_end = cursor;
        (target_start, target_end)
    } else {
        let target_start = cursor;
        let mut nested_parentheses = 0;
        while cursor < bytes.len() {
            match bytes[cursor] {
                b'\\' => cursor = (cursor + 2).min(bytes.len()),
                b'(' => {
                    nested_parentheses += 1;
                    cursor += 1;
                }
                b')' if nested_parentheses == 0 => break,
                b')' => {
                    nested_parentheses -= 1;
                    cursor += 1;
                }
                byte if byte.is_ascii_whitespace() => break,
                _ => cursor += 1,
            }
        }
        (target_start, cursor)
    };

    let link_end = parse_markdown_link_suffix(bytes, cursor)?;
    Some((target_start, target_end, link_end))
}

fn parse_markdown_link_suffix(bytes: &[u8], cursor: usize) -> Option<usize> {
    let mut cursor = skip_whitespace(bytes, cursor);
    match *bytes.get(cursor)? {
        b')' => Some(cursor + 1),
        quote @ (b'"' | b'\'') => {
            cursor += 1;
            while cursor < bytes.len() {
                if bytes[cursor] == b'\\' {
                    cursor = (cursor + 2).min(bytes.len());
                } else if bytes[cursor] == quote {
                    cursor = skip_whitespace(bytes, cursor + 1);
                    return (bytes.get(cursor) == Some(&b')')).then_some(cursor + 1);
                } else {
                    cursor += 1;
                }
            }
            None
        }
        b'(' => {
            let mut depth = 1;
            cursor += 1;
            while cursor < bytes.len() {
                match bytes[cursor] {
                    b'\\' => cursor = (cursor + 2).min(bytes.len()),
                    b'(' => {
                        depth += 1;
                        cursor += 1;
                    }
                    b')' => {
                        depth -= 1;
                        cursor += 1;
                        if depth == 0 {
                            cursor = skip_whitespace(bytes, cursor);
                            return (bytes.get(cursor) == Some(&b')')).then_some(cursor + 1);
                        }
                    }
                    _ => cursor += 1,
                }
            }
            None
        }
        _ => None,
    }
}

fn find_code_span_end(bytes: &[u8], start: usize, tick_count: usize) -> Option<usize> {
    let mut cursor = start + tick_count;
    while cursor < bytes.len() {
        if bytes[cursor] == b'`' && !is_escaped(bytes, cursor) {
            let run_length = count_run(bytes, cursor, b'`');
            if run_length == tick_count {
                return Some(cursor + run_length);
            }
            cursor += run_length;
        } else {
            cursor += 1;
        }
    }
    None
}

fn unescape_markdown_destination(target: &str) -> String {
    let mut chars = target.char_indices().peekable();
    let mut unescaped = String::with_capacity(target.len());
    while let Some((_, character)) = chars.next() {
        if character == '\\'
            && let Some((_, escaped)) = chars.peek().copied()
            && escaped.is_ascii_punctuation()
        {
            let _ = chars.next();
            unescaped.push(escaped);
        } else {
            unescaped.push(character);
        }
    }
    unescaped
}

fn count_run(bytes: &[u8], start: usize, byte: u8) -> usize {
    bytes[start..]
        .iter()
        .take_while(|&&candidate| candidate == byte)
        .count()
}

fn is_escaped(bytes: &[u8], index: usize) -> bool {
    let mut cursor = index;
    while cursor > 0 && bytes[cursor - 1] == b'\\' {
        cursor -= 1;
    }
    (index - cursor) % 2 == 1
}

fn skip_whitespace(bytes: &[u8], mut cursor: usize) -> usize {
    while cursor < bytes.len() && bytes[cursor].is_ascii_whitespace() {
        cursor += 1;
    }
    cursor
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case::resolves_relative_paths(
        "[root](/reports/annual), [relative](./detail), [parent](../archive)",
        "https://news.example.com/reports/section/overview",
        "[root](https://news.example.com/reports/annual), [relative](https://news.example.com/reports/section/detail), [parent](https://news.example.com/reports/archive)"
    )]
    #[case::preserves_absolute_links_and_prose(
        "Text [external](https://docs.example.org/guide) stays as written.",
        "https://news.example.com/reports/section/overview",
        "Text [external](https://docs.example.org/guide) stays as written."
    )]
    #[case::preserves_code_spans(
        "Code `[literal](/ignored)` and [visible](/reports/annual).",
        "https://news.example.com/reports/section/overview",
        "Code `[literal](/ignored)` and [visible](https://news.example.com/reports/annual)."
    )]
    #[case::preserves_angle_targets_titles_and_parentheses(
        "[angle](</reports/annual>), [titled](../archive \"archive\"), [nested](./chart_(daily))",
        "https://news.example.com/reports/section/overview",
        "[angle](<https://news.example.com/reports/annual>), [titled](https://news.example.com/reports/archive \"archive\"), [nested](https://news.example.com/reports/section/chart_(daily))"
    )]
    #[case::unescapes_markdown_destination(
        r"[escaped](/reports/annual\_summary)",
        "https://news.example.com/reports/section/overview",
        "[escaped](https://news.example.com/reports/annual_summary)"
    )]
    #[case::preserves_snippet_when_page_url_is_invalid(
        "[relative](/reports/annual) remains unchanged.",
        "invalid address",
        "[relative](/reports/annual) remains unchanged."
    )]
    fn absolutize_markdown_links_preserves_the_snippet_contract(
        #[case] snippet: &str,
        #[case] page_url: &str,
        #[case] expected: &str,
    ) {
        assert_eq!(absolutize_markdown_links(snippet, page_url), expected,);
    }
}
