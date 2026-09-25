//! ノート本文・図から `[[kind:id]]` 参照を抽出し `note_ref` に同期する。
//!
//! REST の `/api/notes` handler と MCP `write_note` tool の両方から呼ばれる。

use std::ops::Range;

use sea_orm::ActiveValue::Set;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use uuid::Uuid;

use crate::entities::note_ref;
use crate::error::AppError;
use crate::services::graph::GraphDef;

pub(crate) const ALLOWED_REF_KINDS: [&str; 4] = ["stock", "indicator", "sector", "theme"];

/// note_ref を本文 + 図から都度 rebuild する: 旧 ref は DELETE で消え、
/// 本文または graphs[].ref に残るものだけ INSERT 復元する。
pub async fn sync_note_refs<C: sea_orm::ConnectionTrait>(
    db: &C,
    note_id: uuid::Uuid,
    body_md: &str,
    graphs_json: &serde_json::Value,
) -> Result<(), AppError> {
    sync_note_refs_with_policy(db, note_id, body_md, graphs_json, BodyTokenPolicy::Validate).await
}

pub(crate) async fn sync_note_refs_after_graphs_only_update<C: sea_orm::ConnectionTrait>(
    db: &C,
    note_id: uuid::Uuid,
    body_md: &str,
    graphs_json: &serde_json::Value,
) -> Result<(), AppError> {
    sync_note_refs_with_policy(
        db,
        note_id,
        body_md,
        graphs_json,
        BodyTokenPolicy::AllowLegacyBodyTokens,
    )
    .await
}

async fn sync_note_refs_with_policy<C: sea_orm::ConnectionTrait>(
    db: &C,
    note_id: uuid::Uuid,
    body_md: &str,
    graphs_json: &serde_json::Value,
    body_token_policy: BodyTokenPolicy,
) -> Result<(), AppError> {
    let graphs = deserialize_graphs(graphs_json)?;
    let refs_result = match body_token_policy {
        BodyTokenPolicy::Validate => collect_note_refs(body_md, &graphs),
        BodyTokenPolicy::AllowLegacyBodyTokens => {
            collect_note_refs_with_policy(body_md, &graphs, body_token_policy)
        }
    };
    let mut refs =
        refs_result.map_err(|errors| AppError::Validation(format_note_token_errors(&errors)))?;
    refs.sort();
    refs.dedup();

    note_ref::Entity::delete_many()
        .filter(note_ref::Column::NoteId.eq(note_id))
        .exec(db)
        .await?;

    if refs.is_empty() {
        return Ok(());
    }

    let models: Vec<note_ref::ActiveModel> = refs
        .into_iter()
        .map(|(kind, id)| note_ref::ActiveModel {
            note_id: Set(note_id),
            ref_kind: Set(kind),
            ref_id: Set(id),
        })
        .collect();

    note_ref::Entity::insert_many(models)
        .on_conflict(
            sea_orm::sea_query::OnConflict::columns([
                note_ref::Column::NoteId,
                note_ref::Column::RefKind,
                note_ref::Column::RefId,
            ])
            .do_nothing()
            .to_owned(),
        )
        .exec_without_returning(db)
        .await?;
    Ok(())
}

fn deserialize_graphs(graphs_json: &serde_json::Value) -> Result<Vec<GraphDef>, AppError> {
    serde_json::from_value(graphs_json.clone())
        .map_err(|e| AppError::Validation(format!("invalid graphs_json: {e}")))
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
enum NoteTokenValidationError {
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

#[derive(Debug, PartialEq, Eq)]
enum TokenKind {
    Ref(String, String),
    Annotation,
    Graph(String),
    Note(NoteLinkToken),
    Invalid(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct NoteLinkToken {
    pub note_id: Uuid,
    pub follows_current: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BodyTokenPolicy {
    Validate,
    AllowLegacyBodyTokens,
}

#[derive(Debug, Clone, Copy)]
struct NoteToken<'a> {
    inner: &'a str,
    start: usize,
    end: usize,
}

/// frontend の `/\[\[([^\]]+)\]\]/g` と同じく、内側に `]` を含まない範囲を切り出す。
fn extract_tokens(body: &str) -> Vec<NoteToken<'_>> {
    let mut tokens = Vec::new();
    let mut search_from = 0;

    while let Some(open_offset) = body[search_from..].find("[[") {
        let start = search_from + open_offset;
        let inner_start = start + 2;
        let Some(close_offset) = body[inner_start..].find(']') else {
            break;
        };
        let close = inner_start + close_offset;
        let has_closing_pair = body.as_bytes().get(close + 1) == Some(&b']');

        if close > inner_start && has_closing_pair {
            tokens.push(NoteToken {
                inner: &body[inner_start..close],
                start,
                end: close + 2,
            });
            search_from = close + 2;
        } else {
            // 正規表現は失敗した開始位置の次から再検索し、内側の `[[` も見つける。
            search_from = start + 1;
        }
    }

    tokens
}

/// frontend の Markdown AST は code block と inline code を token 置換の対象にしない。
fn markdown_code_ranges(body: &str) -> Vec<Range<usize>> {
    let mut ranges = markdown_code_block_ranges(body);
    ranges.extend(inline_code_ranges(body, &ranges));
    ranges
}

fn markdown_code_block_ranges(body: &str) -> Vec<Range<usize>> {
    let mut ranges = Vec::new();
    let mut fence: Option<(usize, u8, usize)> = None;
    let mut offset = 0;

    for line in body.split_inclusive('\n') {
        let content = line_content(line);
        if let Some((start, marker, minimum_length)) = fence {
            if is_closing_fence(content, marker, minimum_length) {
                ranges.push(start..offset + line.len());
                fence = None;
            }
        } else if let Some((marker, length)) = opening_fence(content) {
            fence = Some((offset, marker, length));
        } else if indentation_columns(content) >= 4 && !content.trim().is_empty() {
            ranges.push(offset..offset + line.len());
        }
        offset += line.len();
    }

    if let Some((start, _, _)) = fence {
        ranges.push(start..body.len());
    }
    ranges
}

fn line_content(line: &str) -> &str {
    let without_newline = line.strip_suffix('\n').unwrap_or(line);
    without_newline
        .strip_suffix('\r')
        .unwrap_or(without_newline)
}

fn opening_fence(line: &str) -> Option<(u8, usize)> {
    let indentation = line.bytes().take_while(|byte| *byte == b' ').count();
    if indentation > 3 {
        return None;
    }
    let content = &line[indentation..];
    let marker = *content.as_bytes().first()?;
    if marker != b'`' && marker != b'~' {
        return None;
    }
    let length = content.bytes().take_while(|byte| *byte == marker).count();
    if length < 3 || (marker == b'`' && content[length..].contains('`')) {
        return None;
    }
    Some((marker, length))
}

fn is_closing_fence(line: &str, marker: u8, minimum_length: usize) -> bool {
    let indentation = line.bytes().take_while(|byte| *byte == b' ').count();
    if indentation > 3 {
        return false;
    }
    let content = &line[indentation..];
    let length = content.bytes().take_while(|byte| *byte == marker).count();
    length >= minimum_length && content[length..].trim().is_empty()
}

fn indentation_columns(line: &str) -> usize {
    line.chars()
        .take_while(|character| *character == ' ' || *character == '\t')
        .fold(0, |columns, character| {
            if character == '\t' {
                (columns / 4 + 1) * 4
            } else {
                columns + 1
            }
        })
}

fn inline_code_ranges(body: &str, code_blocks: &[Range<usize>]) -> Vec<Range<usize>> {
    let mut ranges = Vec::new();
    let mut search_from = 0;

    while let Some(open_offset) = body[search_from..].find('`') {
        let start = search_from + open_offset;
        if let Some(block) = range_containing(code_blocks, start) {
            search_from = block.end;
            continue;
        }

        let delimiter_length = backtick_run_length(body, start);
        let delimiter_end = start + delimiter_length;
        let mut close_search_from = delimiter_end;
        let mut close = None;

        while let Some(close_offset) = body[close_search_from..].find('`') {
            let close_start = close_search_from + close_offset;
            if has_blank_line(body, start, close_start)
                || code_blocks
                    .iter()
                    .any(|block| block.start > start && block.start < close_start)
            {
                break;
            }
            if range_containing(code_blocks, close_start).is_some() {
                break;
            }
            let close_length = backtick_run_length(body, close_start);
            if close_length == delimiter_length {
                close = Some(close_start + close_length);
                break;
            }
            close_search_from = close_start + close_length;
        }

        if let Some(end) = close {
            ranges.push(start..end);
            search_from = end;
        } else {
            search_from = delimiter_end;
        }
    }

    ranges
}

fn has_blank_line(body: &str, start: usize, end: usize) -> bool {
    body[start..end]
        .split_inclusive('\n')
        .any(|line| line_content(line).trim().is_empty())
}

fn backtick_run_length(body: &str, start: usize) -> usize {
    body.as_bytes()[start..]
        .iter()
        .take_while(|byte| **byte == b'`')
        .count()
}

fn range_containing(ranges: &[Range<usize>], position: usize) -> Option<&Range<usize>> {
    ranges
        .iter()
        .find(|range| position >= range.start && position < range.end)
}

fn classify_token(inner: &str) -> TokenKind {
    let Some((kind, id)) = inner.split_once(':') else {
        return TokenKind::Invalid("kind:id の形式で prefix を指定してください".to_string());
    };

    if ALLOWED_REF_KINDS.contains(&kind) {
        let id = id.trim();
        if id.is_empty() {
            return TokenKind::Invalid("参照 ID を空にできません".to_string());
        }
        return TokenKind::Ref(kind.to_string(), id.to_string());
    }

    if kind == "note" {
        let (id, follows_current) = match id.strip_suffix("@current") {
            Some(id) => (id, true),
            None => (id, false),
        };
        let Ok(note_id) = Uuid::parse_str(id) else {
            return TokenKind::Invalid("ノート ID は UUID で指定してください".to_string());
        };
        if !note_id.to_string().eq_ignore_ascii_case(id) {
            return TokenKind::Invalid(
                "ノート ID は標準形式の UUID で指定してください".to_string(),
            );
        }
        return TokenKind::Note(NoteLinkToken {
            note_id,
            follows_current,
        });
    }

    if kind == "anno" {
        if is_valid_token_id(id, false) {
            return TokenKind::Annotation;
        }
        return TokenKind::Invalid(
            "annotation ID は英数字で始まり、英数字・`_`・`-` のみ使用できます".to_string(),
        );
    }

    if kind == "graph" {
        if is_valid_token_id(id, true) {
            return TokenKind::Graph(id.to_string());
        }
        return TokenKind::Invalid(
            "graph ID は英字で始まり、英数字・`_`・`-` のみ使用できます".to_string(),
        );
    }

    TokenKind::Invalid(format!("未知の prefix `{kind}` です"))
}

pub(crate) fn extract_note_link_tokens(body: &str) -> Vec<NoteLinkToken> {
    tokens_outside_code(body)
        .into_iter()
        .filter_map(|token| match classify_token(token.inner) {
            TokenKind::Note(note_link) => Some(note_link),
            _ => None,
        })
        .collect()
}

fn tokens_outside_code(body: &str) -> Vec<NoteToken<'_>> {
    let code_ranges = markdown_code_ranges(body);
    extract_tokens(body)
        .into_iter()
        .filter(|token| {
            !code_ranges
                .iter()
                .any(|range| token.start >= range.start && token.end <= range.end)
        })
        .collect()
}

/// frontend の anno / graph token matcher と同じ ID 文字を受け付ける。
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

fn collect_note_refs(
    body: &str,
    graphs: &[GraphDef],
) -> Result<Vec<(String, String)>, Vec<NoteTokenValidationError>> {
    collect_note_refs_with_policy(body, graphs, BodyTokenPolicy::Validate)
}

fn collect_note_refs_with_policy(
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
            TokenKind::Ref(kind, id) => refs.push((kind, id)),
            TokenKind::Annotation | TokenKind::Note(_) => {}
            TokenKind::Graph(id) => {
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
            TokenKind::Invalid(reason) => {
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
                TokenKind::Ref(kind, id) => {
                    refs.push((kind, id));
                    continue;
                }
                TokenKind::Annotation | TokenKind::Graph(_) | TokenKind::Note(_) => {
                    "図ノードでは stock / indicator / sector / theme の参照だけを使用できます"
                        .to_string()
                }
                TokenKind::Invalid(reason) => {
                    format!(
                        "{reason}; 図ノードでは stock / indicator / sector / theme の参照だけを使用できます"
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

fn format_note_token_errors(errors: &[NoteTokenValidationError]) -> String {
    let details = errors
        .iter()
        .map(|error| format!("- {error}"))
        .collect::<Vec<_>>()
        .join("\n");
    indoc::formatdoc! {"
        ノートのトークンに問題があります:
        {details}
        許可される形式: `[[stock:<id>]]`, `[[indicator:<id>]]`, `[[sector:<id>]]`, `[[theme:<id>]]`, `[[note:<uuid>]]`, `[[note:<uuid>@current]]`, `[[anno:<id>]]`。`[[graph:<id>]]` は graphs[].id に存在し、空行区切りブロック内で単独にしてください。graphs[].nodes[].ref では参照 4 種のみ使用できます。
    "}
    .trim_end()
    .to_string()
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;
    use crate::services::graph::{GraphNode, Layout};

    fn graph(id: &str, node_ref: Option<&str>) -> GraphDef {
        GraphDef {
            id: id.to_string(),
            layout: Layout::Flow,
            title: None,
            nodes: vec![GraphNode {
                id: "node-1".to_string(),
                label: "node".to_string(),
                r#ref: node_ref.map(str::to_string),
                value: None,
                cite: None,
                parent: None,
                x: None,
                y: None,
            }],
            edges: Vec::new(),
        }
    }

    #[rstest]
    #[case::array_literal("[[1, 2], [3, 4]]", vec![])]
    #[case::malformed_before_valid("[[bad] then [[stock:demo-code]]", vec!["[[stock:demo-code]]"])]
    #[case::adjacent("[[stock:demo-code]][[theme:demo-theme]]", vec!["[[stock:demo-code]]", "[[theme:demo-theme]]"])]
    #[case::unclosed("[[unfinished", vec![])]
    #[case::empty_inner("[[]]", vec![])]
    #[case::emptyish_inner("[[ ]]", vec!["[[ ]]"])]
    #[case::newline_inside("[[stock:\ndemo-code]]", vec!["[[stock:\ndemo-code]]"])]
    fn test_extract_tokens_matches_frontend_shape(#[case] body: &str, #[case] expected: Vec<&str>) {
        let got = extract_tokens(body)
            .iter()
            .map(|token| &body[token.start..token.end])
            .collect::<Vec<_>>();
        assert_eq!(got, expected);
    }

    #[test]
    fn test_collect_note_refs_accepts_reference_annotation_graph_and_array_literal() {
        let body = concat!(
            "[[stock:demo-code]] [[indicator:demo-index]] [[sector:demo-sector]] [[theme:demo-theme]] ",
            "[[anno:a-1]]\n\n [[graph:g1]] \n\n[[1, 2], [3, 4]]",
        );
        assert_eq!(
            collect_note_refs(body, &[graph("g1", Some("stock:demo-code"))]),
            Ok(vec![
                ("stock".to_string(), "demo-code".to_string()),
                ("indicator".to_string(), "demo-index".to_string()),
                ("sector".to_string(), "demo-sector".to_string()),
                ("theme".to_string(), "demo-theme".to_string()),
                ("stock".to_string(), "demo-code".to_string()),
            ]),
        );
    }

    #[rstest]
    #[case::inline_code("inline `[[foo:bar]]` code")]
    #[case::multiline_inline_code("`[[foo:bar]]\n[[stock:demo-code]]`")]
    #[case::backtick_fence(indoc::indoc! {"
        ```text
        [[foo:bar]]
        ```
    "})]
    #[case::tilde_fence(indoc::indoc! {"
        ~~~text
        [[graph:g1]]
        ~~~
    "})]
    #[case::indented_code("    [[foo:bar]]")]
    fn test_collect_note_refs_ignores_markdown_code(#[case] body: &str) {
        assert_eq!(collect_note_refs(body, &[]), Ok(vec![]));
    }

    #[test]
    fn test_collect_note_refs_does_not_extend_inline_code_across_blank_lines() {
        let body = indoc::indoc! {"
            `unclosed

            [[foo:bar]]
        "};
        assert_eq!(
            collect_note_refs(body, &[]).map_err(|_| "invalid"),
            Err("invalid")
        );
    }

    #[test]
    fn test_collect_note_refs_reports_every_invalid_token_and_allowed_form() {
        let body = indoc::indoc! {"
            [[foo:bar]] [[bare-demo]] [[stock:]] [[anno:]] [[graph:1bad]]

            [[graph:missing]]

            mixed [[graph:g1]]
        "};
        let errors = collect_note_refs(body, &[graph("g1", Some("foo:bar"))])
            .expect_err("invalid note tokens should be rejected");
        assert_eq!(
            format_note_token_errors(&errors),
            concat!(
                "ノートのトークンに問題があります:\n",
                "- 本文のトークン \"[[foo:bar]]\": 未知の prefix `foo` です\n",
                "- 本文のトークン \"[[bare-demo]]\": kind:id の形式で prefix を指定してください\n",
                "- 本文のトークン \"[[stock:]]\": 参照 ID を空にできません\n",
                "- 本文のトークン \"[[anno:]]\": annotation ID は英数字で始まり、英数字・`_`・`-` のみ使用できます\n",
                "- 本文のトークン \"[[graph:1bad]]\": graph ID は英字で始まり、英数字・`_`・`-` のみ使用できます\n",
                "- 本文のトークン \"[[graph:missing]]\": 対応する graphs[].id がありません\n",
                "- 本文のトークン \"[[graph:g1]]\": 図トークンは空行区切りブロック内で単独にしてください\n",
                "- graphs[0].nodes[0].ref の値 \"[[foo:bar]]\": 未知の prefix `foo` です; 図ノードでは stock / indicator / sector / theme の参照だけを使用できます\n",
                "許可される形式: `[[stock:<id>]]`, `[[indicator:<id>]]`, `[[sector:<id>]]`, `[[theme:<id>]]`, `[[note:<uuid>]]`, `[[note:<uuid>@current]]`, `[[anno:<id>]]`。`[[graph:<id>]]` は graphs[].id に存在し、空行区切りブロック内で単独にしてください。graphs[].nodes[].ref では参照 4 種のみ使用できます。",
            ),
        );
    }

    #[test]
    fn test_deserialize_graphs_rejects_malformed_graphs_json() {
        let got = deserialize_graphs(&serde_json::json!([{"id": "g1"}])).unwrap_err();
        assert_eq!(
            got.to_string(),
            "validation error: invalid graphs_json: missing field `layout`",
        );
    }

    #[rstest]
    #[case::mixed_paragraph("text [[graph:g1]]", 1)]
    #[case::same_block_multiple("[[graph:g1]]\n[[graph:g1]]", 2)]
    fn test_collect_note_refs_rejects_non_standalone_graph_tokens(
        #[case] body: &str,
        #[case] error_count: usize,
    ) {
        assert_eq!(
            collect_note_refs(body, &[graph("g1", None)]),
            Err(vec![
                NoteTokenValidationError::GraphToken {
                    token: "[[graph:g1]]".to_string(),
                    reason: "図トークンは空行区切りブロック内で単独にしてください".to_string(),
                };
                error_count
            ]),
        );
    }

    #[rstest]
    #[case::with_ref(
        graph("g1", Some("stock:demo-code")),
        Ok(vec![("stock".to_string(), "demo-code".to_string())])
    )]
    #[case::no_ref(graph("g1", None), Ok(vec![]))]
    #[case::invalid_kind(graph("g1", Some("foo:bar")), Err("invalid"))]
    #[case::annotation_not_allowed(graph("g1", Some("anno:a1")), Err("invalid"))]
    fn test_collect_note_refs_validates_graph_node_refs(
        #[case] graph: GraphDef,
        #[case] expected: Result<Vec<(String, String)>, &str>,
    ) {
        let got = collect_note_refs("", &[graph]);
        assert_eq!(got.map_err(|_| "invalid"), expected);
    }

    #[sqlx::test(migrations = false)]
    async fn sync_note_refs_indexes_refs_from_body_and_graphs_without_duplication(
        pool: sqlx::PgPool,
    ) {
        let db = crate::testing::create_test_db(pool).await;
        let strategy_id = crate::testing::insert_test_strategy(&db, "s").await;
        let note_id = crate::testing::insert_test_note(&db, strategy_id, "t", "orig").await;

        let graphs_json = serde_json::json!([{
            "id": "g1",
            "layout": "flow",
            "title": null,
            "nodes": [
                {
                    "id": "n1", "label": "node", "ref": "stock:demo-code",
                    "value": null, "cite": null, "parent": null, "x": null, "y": null,
                },
                {
                    "id": "n2", "label": "node", "ref": "stock:demo-code",
                    "value": null, "cite": null, "parent": null, "x": null, "y": null,
                },
            ],
            "edges": [],
        }]);

        sync_note_refs(
            &db,
            note_id,
            "body mentions [[stock:demo-code]] and [[theme:demo-theme]]",
            &graphs_json,
        )
        .await
        .unwrap();

        let mut refs = note_ref::Entity::find()
            .filter(note_ref::Column::NoteId.eq(note_id))
            .all(&db)
            .await
            .unwrap();
        refs.sort_by(|a, b| (&a.ref_kind, &a.ref_id).cmp(&(&b.ref_kind, &b.ref_id)));

        assert_eq!(
            refs.into_iter()
                .map(|r| (r.ref_kind, r.ref_id))
                .collect::<Vec<_>>(),
            vec![
                ("stock".to_string(), "demo-code".to_string()),
                ("theme".to_string(), "demo-theme".to_string()),
            ],
        );
    }
}
