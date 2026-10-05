use std::ops::Range;

use uuid::Uuid;

use super::note_graph::GraphDef;

mod markdown;

pub(crate) use markdown::prose_segments;
pub(crate) use markdown::tokens_outside_code;
pub use markdown::{NoteToken, extract_tokens};

pub const ALLOWED_REF_KINDS: [&str; 3] = ["stock", "indicator", "group"];

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum NoteTokenValidationError {
    #[error("本文のトークン {token:?}: {reason}")]
    BodyToken { token: String, reason: String },
    #[error("本文のトークン {token:?}: {reason}")]
    GraphToken { token: String, reason: String },
    #[error("{location} の値 {token:?}: {reason}")]
    GraphRef {
        location: String,
        token: String,
        reason: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NoteLinkToken {
    pub note_id: Uuid,
    pub follows_current: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BodyTokenPolicy {
    Validate,
    AllowLegacyBodyTokens,
}

enum TokenClassification<'a> {
    Ref(&'a str, String),
    PriceReference,
    RemovedRef,
    Annotation,
    Graph(String),
    Note(NoteLinkToken),
    Invalid(String),
}

fn classify_token(inner: &str) -> TokenClassification<'_> {
    let Some((kind, id)) = inner.split_once(':') else {
        return TokenClassification::Invalid(
            "kind:id の形式で prefix を指定してください".to_string(),
        );
    };

    if matches!(kind, "sector" | "theme") {
        if id.trim().is_empty() {
            return TokenClassification::Invalid("参照 ID を空にできません".to_string());
        }
        return TokenClassification::RemovedRef;
    }

    if ALLOWED_REF_KINDS.contains(&kind) {
        let id = id.trim();
        if id.is_empty() {
            return TokenClassification::Invalid("参照 ID を空にできません".to_string());
        }
        if !is_valid_ref_id_format(kind, id) {
            return TokenClassification::Invalid(
                "group ID は axis-key/group-key 形式で指定してください".to_string(),
            );
        }
        return TokenClassification::Ref(kind, id.to_string());
    }

    if kind == "note" {
        let (id, follows_current) = match id.strip_suffix("@current") {
            Some(id) => (id, true),
            None => (id, false),
        };
        let Ok(note_id) = Uuid::parse_str(id) else {
            return TokenClassification::Invalid(
                "ノート ID は UUID で指定してください".to_string(),
            );
        };
        if !note_id.to_string().eq_ignore_ascii_case(id) {
            return TokenClassification::Invalid(
                "ノート ID は標準形式の UUID で指定してください".to_string(),
            );
        }
        return TokenClassification::Note(NoteLinkToken {
            note_id,
            follows_current,
        });
    }

    if matches!(kind, "price" | "change") {
        return TokenClassification::PriceReference;
    }

    if kind == "anno" {
        if is_valid_token_id(id, false) {
            return TokenClassification::Annotation;
        }
        return TokenClassification::Invalid(
            "annotation ID は英数字で始まり、英数字・`_`・`-` のみ使用できます".to_string(),
        );
    }

    if kind == "graph" {
        if is_valid_token_id(id, true) {
            return TokenClassification::Graph(id.to_string());
        }
        return TokenClassification::Invalid(
            "graph ID は英字で始まり、英数字・`_`・`-` のみ使用できます".to_string(),
        );
    }

    TokenClassification::Invalid(format!("未知の prefix `{kind}` です"))
}

pub fn extract_note_link_tokens(body: &str) -> Vec<NoteLinkToken> {
    tokens_outside_code(body)
        .into_iter()
        .filter_map(|token| match classify_token(token.inner) {
            TokenClassification::Note(note_link) => Some(note_link),
            _ => None,
        })
        .collect()
}

fn is_valid_token_id(id: &str, alphabetic_start: bool) -> bool {
    let mut chars = id.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    let valid_first = if alphabetic_start {
        first.is_ascii_alphabetic()
    } else {
        first.is_ascii_alphanumeric()
    };
    valid_first && chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

pub fn is_valid_group_ref_id(id: &str) -> bool {
    let Some((axis_key, group_key)) = id.split_once('/') else {
        return false;
    };
    !axis_key.is_empty() && !group_key.is_empty() && !group_key.contains('/')
}

/// group 参照は軸 key と group key を含む。他の参照 ID は不透明値として扱う。
pub fn is_valid_ref_id_format(kind: &str, id: &str) -> bool {
    match kind {
        "stock" | "indicator" => true,
        "group" => is_valid_group_ref_id(id),
        _ => false,
    }
}

pub fn collect_note_refs(
    body: &str,
    graphs: &[GraphDef],
) -> Result<Vec<(String, String)>, Vec<NoteTokenValidationError>> {
    collect_note_refs_with_policy(body, graphs, BodyTokenPolicy::Validate)
}

pub fn collect_note_refs_with_policy(
    body: &str,
    graphs: &[GraphDef],
    body_token_policy: BodyTokenPolicy,
) -> Result<Vec<(String, String)>, Vec<NoteTokenValidationError>> {
    let mut refs = Vec::new();
    let mut errors = Vec::new();
    let graph_blocks = blank_line_blocks(body);
    for token in tokens_outside_code(body) {
        let token_text = &body[token.start..token.end];
        match classify_token(token.inner) {
            TokenClassification::Ref(kind, id) => refs.push((kind.to_string(), id)),
            TokenClassification::RemovedRef => {}
            TokenClassification::Annotation
            | TokenClassification::Note(_)
            | TokenClassification::PriceReference => {}
            TokenClassification::Graph(id) => {
                let mut reasons = Vec::new();
                if !is_standalone_graph_token(body, token, &graph_blocks) {
                    reasons.push("図トークンは空行区切りブロック内で単独にしてください");
                }
                if !graphs.iter().any(|graph| graph.id == id) {
                    reasons.push("対応する graphs[].id がありません");
                }
                if !reasons.is_empty() {
                    errors.push(NoteTokenValidationError::GraphToken {
                        token: token_text.to_string(),
                        reason: reasons.join("; "),
                    });
                }
            }
            TokenClassification::Invalid(reason) => {
                if body_token_policy == BodyTokenPolicy::Validate {
                    errors.push(NoteTokenValidationError::BodyToken {
                        token: token_text.to_string(),
                        reason,
                    });
                }
            }
        }
    }

    for (graph_index, graph) in graphs.iter().enumerate() {
        for (node_index, node) in graph.nodes.iter().enumerate() {
            let Some(value) = node.r#ref.as_deref() else {
                continue;
            };
            let location = format!("graphs[{graph_index}].nodes[{node_index}].ref");
            let reason = match classify_token(value) {
                TokenClassification::Ref(kind, id) => {
                    refs.push((kind.to_string(), id));
                    continue;
                }
                TokenClassification::Annotation
                | TokenClassification::Graph(_)
                | TokenClassification::Note(_)
                | TokenClassification::PriceReference
                | TokenClassification::RemovedRef => {
                    "図ノードでは stock / indicator / group の参照だけを使用できます".to_string()
                }
                TokenClassification::Invalid(reason) => {
                    format!(
                        "{reason}; 図ノードでは stock / indicator / group の参照だけを使用できます"
                    )
                }
            };
            errors.push(NoteTokenValidationError::GraphRef {
                location,
                token: format!("[[{value}]]"),
                reason,
            });
        }
    }

    if errors.is_empty() {
        Ok(refs)
    } else {
        Err(errors)
    }
}

fn blank_line_blocks(body: &str) -> Vec<Range<usize>> {
    let mut blocks = Vec::new();
    let mut block_start = 0;
    let mut offset = 0;

    for line in body.split_inclusive('\n') {
        let without_newline = line.strip_suffix('\n').unwrap_or(line);
        let content = without_newline
            .strip_suffix('\r')
            .unwrap_or(without_newline);
        if content.trim().is_empty() {
            if block_start < offset {
                blocks.push(block_start..offset);
            }
            block_start = offset + line.len();
        }
        offset += line.len();
    }
    if block_start < body.len() {
        blocks.push(block_start..body.len());
    }

    blocks
}

fn is_standalone_graph_token(body: &str, token: NoteToken<'_>, blocks: &[Range<usize>]) -> bool {
    let Some(block) = blocks
        .iter()
        .find(|block| token.start >= block.start && token.end <= block.end)
    else {
        return false;
    };
    let block_text = &body[block.clone()];
    if block_text.trim() != &body[token.start..token.end] {
        return false;
    }

    let line_start = body[..token.start]
        .rfind('\n')
        .map_or(block.start, |index| index + 1);
    let indentation = &body[line_start..token.start];
    let indentation_columns = indentation
        .chars()
        .take_while(|c| *c == ' ' || *c == '\t')
        .fold(0, |columns, c| {
            if c == '\t' {
                (columns / 4 + 1) * 4
            } else {
                columns + 1
            }
        });
    indentation_columns < 4
}

pub fn format_note_token_errors(errors: &[NoteTokenValidationError]) -> String {
    let details = errors
        .iter()
        .map(|error| format!("- {error}"))
        .collect::<Vec<_>>()
        .join("\n");
    let mut message = String::new();
    message.push_str("ノートのトークンに問題があります:");
    message.push('\n');
    message.push_str(&details);
    message.push('\n');
    message.push_str("許可される形式: `[[stock:<id>]]`, `[[indicator:<id>]]`, `[[group:<axis-key>/<group-key>]]`, `[[note:<uuid>]]`, `[[note:<uuid>@current]]`, `[[anno:<id>]]`, `[[price:<id>@<date>:<field>]]`, `[[change:<id>@<start>..<end>:<field>]]`。`[[graph:<id>]]` は graphs[].id に存在し、空行区切りブロック内で単独にしてください。graphs[].nodes[].ref では参照 3 種のみ使用できます。");
    message
}

#[cfg(test)]
mod tests {
    use indoc::indoc;
    use rstest::rstest;

    use super::collect_note_refs;

    #[test]
    fn foreign_stock_link_keeps_country_code_in_reference_id() {
        assert_eq!(
            collect_note_refs("[[stock:KR:QZ9012]]", &[]),
            Ok(vec![("stock".into(), "KR:QZ9012".into())]),
        );
    }

    #[rstest]
    #[case::inline_code("inline `[[sample:token]]` code")]
    #[case::multiline_inline_code(indoc! {"
        `[[sample:token]]
        [[stock:sample-code]]`
    "})]
    #[case::backtick_fence(indoc! {"
        ```text
        [[sample:token]]
        ```
    "})]
    #[case::tilde_fence(indoc! {"
        ~~~text
        [[graph:sample-graph]]
        ~~~
    "})]
    #[case::indented_code("    [[sample:token]]")]
    fn markdown_code_does_not_produce_note_references(#[case] body: &str) {
        assert_eq!(collect_note_refs(body, &[]), Ok(vec![]));
    }
}
