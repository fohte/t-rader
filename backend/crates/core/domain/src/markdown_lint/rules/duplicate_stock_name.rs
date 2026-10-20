use std::ops::Range;

use unicode_normalization::char::{canonical_combining_class, is_combining_mark};

use super::super::normalization::nfkc_lowercase;
use super::super::{
    MarkdownLintContext, RuleDefinition, RuleFinding, RuleRequirements, Severity, TextEdit,
};

const MESSAGE: &str = "銘柄リンク直後の同名を削除してください。";

pub static RULE: RuleDefinition = RuleDefinition::new(
    "duplicate-stock-name",
    Severity::Error,
    RuleRequirements {
        resolved_reference_names: true,
        ..RuleRequirements::NONE
    },
    check,
);

fn check(context: &MarkdownLintContext<'_>) -> Vec<RuleFinding> {
    let Some(reference_names) = context.resolved_reference_names else {
        return Vec::new();
    };
    let source = context.document.source();
    let mut findings = Vec::new();

    for token in context.document.validated_tokens() {
        let Some(token_text) = source.get(token.range.clone()) else {
            continue;
        };
        let Some(token_inner) = token_text
            .strip_prefix("[[")
            .and_then(|inner| inner.strip_suffix("]]"))
        else {
            continue;
        };
        if !token_inner.starts_with("stock:") {
            continue;
        }
        let Some(Some(reference_name)) = reference_names.get(token_inner) else {
            continue;
        };
        let Some(run) = context.document.plain_text_run_containing(&token.range) else {
            continue;
        };

        let line_end = line_end(source, token.range.end, source.len());
        let run_end = run.range.end.min(line_end);
        let Some(first_name_start) = skip_spaces_and_tabs(source, token.range.end, run_end) else {
            continue;
        };
        let Some(first_name_end) =
            matching_name_end(source, first_name_start, run_end, line_end, reference_name)
        else {
            continue;
        };
        if range_overlaps_url(context, &(first_name_start..first_name_end)) {
            continue;
        }

        let mut duplicate_end = first_name_end;
        let mut next_name_start =
            skip_spaces_and_tabs(source, duplicate_end, run_end).unwrap_or(duplicate_end);
        while next_name_start < run_end
            && let Some(next_name_end) =
                matching_name_end(source, next_name_start, run_end, line_end, reference_name)
            && !range_overlaps_url(context, &(next_name_start..next_name_end))
        {
            duplicate_end = next_name_end;
            next_name_start =
                skip_spaces_and_tabs(source, duplicate_end, run_end).unwrap_or(duplicate_end);
        }

        findings.push(RuleFinding {
            key: token_inner.to_string(),
            range: first_name_start..duplicate_end,
            message: MESSAGE.to_string(),
            fix: Some(TextEdit {
                range: token.range.end..duplicate_end,
                replacement: String::new(),
            }),
        });
    }

    findings
}

fn range_overlaps_url(context: &MarkdownLintContext<'_>, range: &Range<usize>) -> bool {
    context
        .document
        .url_ranges()
        .iter()
        .any(|url| range.start < url.end && url.start < range.end)
}

fn line_end(source: &str, start: usize, limit: usize) -> usize {
    source[start..limit]
        .char_indices()
        .find(|(_, character)| matches!(*character, '\r' | '\n'))
        .map_or(limit, |(offset, _)| start + offset)
}

fn skip_spaces_and_tabs(source: &str, start: usize, limit: usize) -> Option<usize> {
    if start >= limit {
        return None;
    }

    let mut cursor = start;
    while cursor < limit {
        let character = source[cursor..limit].chars().next()?;
        if !matches!(character, ' ' | '\t') {
            break;
        }
        cursor += character.len_utf8();
    }
    (cursor < limit).then_some(cursor)
}

fn matching_name_end(
    source: &str,
    start: usize,
    match_limit: usize,
    line_limit: usize,
    reference_name: &str,
) -> Option<usize> {
    let expected = nfkc_lowercase(reference_name);
    if expected.is_empty() {
        return None;
    }

    for (offset, character) in source[start..match_limit].char_indices() {
        let end = start + offset + character.len_utf8();
        let normalized_candidate = nfkc_lowercase(&source[start..end]);
        if normalized_candidate == expected {
            return has_name_boundary(source, end, line_limit).then_some(end);
        }
        if !expected.starts_with(&normalized_candidate)
            && !source[end..match_limit]
                .chars()
                .next()
                .is_some_and(|next| canonical_combining_class(next) != 0)
        {
            return None;
        }
    }
    None
}

fn has_name_boundary(source: &str, end: usize, line_limit: usize) -> bool {
    if end == line_limit {
        return true;
    }
    let Some(next) = source[end..line_limit].chars().next() else {
        return true;
    };
    !next.is_alphanumeric() && next != '_' && !is_combining_mark(next)
}

#[cfg(test)]
mod tests {
    use std::ops::Range;

    use rstest::rstest;

    use super::RULE;
    use crate::markdown_lint::{
        LintDiagnostic, ParsedMarkdown, ResolvedReferenceNames, RuleRequirements, Severity,
        TextEdit, TextRun, TextRunKind, ValidatedTokenSpan, apply_safe_edits, lint,
    };
    use crate::note_reference::validated_token_ranges;

    #[derive(Clone, Copy)]
    enum NameResolution<'a> {
        Resolved(&'a str),
        Unresolved,
        Missing,
    }

    #[derive(Clone, Copy)]
    enum RunLayout {
        Plain,
        TokenIs(TextRunKind),
        FormattedToken,
        FormattedCandidate,
    }

    fn diagnostic(id: &str, range: Range<usize>, fix_range: Range<usize>) -> LintDiagnostic {
        LintDiagnostic {
            rule_id: "duplicate-stock-name",
            id: id.to_string(),
            severity: Severity::Error,
            range,
            message: "銘柄リンク直後の同名を削除してください。".to_string(),
            fix: Some(TextEdit {
                range: fix_range,
                replacement: String::new(),
            }),
        }
    }

    fn lint_source(
        source: &str,
        resolution: NameResolution<'_>,
        layout: RunLayout,
        url_ranges: Vec<Range<usize>>,
    ) -> Vec<LintDiagnostic> {
        let tokens = validated_token_ranges(source)
            .into_iter()
            .filter_map(|range| {
                let inner = &source[range.start + 2..range.end - 2];
                inner
                    .starts_with("stock:")
                    .then_some(ValidatedTokenSpan { range })
            })
            .collect::<Vec<_>>();
        let runs = runs_for(source, &tokens, layout);
        let document = ParsedMarkdown::new(source, runs, tokens.clone(), url_ranges)
            .expect("synthetic Markdown ranges are valid");
        let mut resolved_names = ResolvedReferenceNames::new();
        for token in &tokens {
            let key = &source[token.range.start + 2..token.range.end - 2];
            match resolution {
                NameResolution::Resolved(name) => {
                    resolved_names.insert(key.to_string(), Some(name.to_string()));
                }
                NameResolution::Unresolved => {
                    resolved_names.insert(key.to_string(), None);
                }
                NameResolution::Missing => {}
            }
        }

        lint(&document, &[&RULE], Some(&resolved_names))
            .expect("the reference names context is present")
            .diagnostics
    }

    fn runs_for(source: &str, tokens: &[ValidatedTokenSpan], layout: RunLayout) -> Vec<TextRun> {
        let Some(token) = tokens.first() else {
            return Vec::new();
        };
        match layout {
            RunLayout::Plain => vec![text_run(0..source.len(), TextRunKind::Text, 0)],
            RunLayout::TokenIs(kind) => {
                let mut runs = Vec::new();
                if token.range.start > 0 {
                    runs.push(text_run(0..token.range.start, TextRunKind::Text, 0));
                }
                runs.push(text_run(token.range.clone(), kind, 0));
                if token.range.end < source.len() {
                    runs.push(text_run(
                        token.range.end..source.len(),
                        TextRunKind::Text,
                        0,
                    ));
                }
                runs
            }
            RunLayout::FormattedToken => {
                let mut runs = Vec::new();
                if token.range.start > 0 {
                    runs.push(text_run(0..token.range.start, TextRunKind::Text, 0));
                }
                runs.push(text_run(token.range.clone(), TextRunKind::Text, 1));
                if token.range.end < source.len() {
                    runs.push(text_run(
                        token.range.end..source.len(),
                        TextRunKind::Text,
                        0,
                    ));
                }
                runs
            }
            RunLayout::FormattedCandidate => {
                let marker_start = source.find("**").expect("formatting marker is present");
                let name_start = source
                    .find("Demo Works")
                    .expect("formatted name is present");
                let name_end = name_start + "Demo Works".len();
                vec![
                    text_run(0..marker_start, TextRunKind::Text, 0),
                    text_run(name_start..name_end, TextRunKind::Text, 1),
                ]
            }
        }
    }

    fn text_run(range: Range<usize>, kind: TextRunKind, formatting_depth: usize) -> TextRun {
        TextRun {
            range,
            kind,
            quote_depth: 0,
            table_cell: None,
            formatting_depth,
        }
    }

    #[test]
    fn declares_reference_name_requirement() {
        assert_eq!(
            crate::markdown_lint::requirements(&[&RULE]),
            RuleRequirements {
                resolved_reference_names: true,
                ..RuleRequirements::NONE
            },
        );
    }

    #[rstest]
    #[case::nfkc_full_width(
        "[[stock:DEMO-A]] ＤＥＭＯ工業:",
        NameResolution::Resolved("Demo工業"),
        vec![diagnostic(
            "duplicate-stock-name:stock%3ADEMO-A:1",
            17..35,
            16..35,
        )],
    )]
    #[case::ascii_case(
        "[[stock:DEMO-B]] dEmO pRoDuCtS: メモ",
        NameResolution::Resolved("Demo Products"),
        vec![diagnostic(
            "duplicate-stock-name:stock%3ADEMO-B:1",
            17..30,
            16..30,
        )],
    )]
    #[case::unicode_case(
        "[[stock:DEMO-C]] École:",
        NameResolution::Resolved("éCOLE"),
        vec![diagnostic(
            "duplicate-stock-name:stock%3ADEMO-C:1",
            17..23,
            16..23,
        )],
    )]
    fn finds_normalized_duplicate_names(
        #[case] source: &str,
        #[case] resolution: NameResolution<'_>,
        #[case] expected: Vec<LintDiagnostic>,
    ) {
        assert_eq!(
            lint_source(source, resolution, RunLayout::Plain, Vec::new()),
            expected,
        );
    }

    #[rstest]
    #[case::repeated_stock_token(
        "[[stock:DEMO-A]] Demo Works; [[stock:DEMO-A]] Demo Works.",
        NameResolution::Resolved("Demo Works"),
        vec![
            diagnostic(
                "duplicate-stock-name:stock%3ADEMO-A:1",
                17..27,
                16..27,
            ),
            diagnostic(
                "duplicate-stock-name:stock%3ADEMO-A:2",
                46..56,
                45..56,
            ),
        ],
    )]
    #[case::contiguous_repetitions(
        "[[stock:DEMO-A]] Demo Works\tDemo Works Demo Works: メモ",
        NameResolution::Resolved("Demo Works"),
        vec![diagnostic(
            "duplicate-stock-name:stock%3ADEMO-A:1",
            17..49,
            16..49,
        )],
    )]
    fn returns_all_occurrences_and_combines_contiguous_repetitions(
        #[case] source: &str,
        #[case] resolution: NameResolution<'_>,
        #[case] expected: Vec<LintDiagnostic>,
    ) {
        assert_eq!(
            lint_source(source, resolution, RunLayout::Plain, Vec::new()),
            expected,
        );
    }

    #[rstest]
    #[case::ascii_partial_match(
        "[[stock:DEMO-A]] Demo Workshop:",
        NameResolution::Resolved("Demo Works"),
        RunLayout::Plain,
        Vec::new()
    )]
    #[case::japanese_partial_match(
        "[[stock:DEMO-A]] 架空工業団地:",
        NameResolution::Resolved("架空工業"),
        RunLayout::Plain,
        Vec::new()
    )]
    #[case::underscore_continuation(
        "[[stock:DEMO-A]] Demo Works_extra:",
        NameResolution::Resolved("Demo Works"),
        RunLayout::Plain,
        Vec::new()
    )]
    #[case::unknown_name(
        "[[stock:DEMO-A]] Demo Works:",
        NameResolution::Unresolved,
        RunLayout::Plain,
        Vec::new()
    )]
    #[case::missing_name_fact(
        "[[stock:DEMO-A]] Demo Works:",
        NameResolution::Missing,
        RunLayout::Plain,
        Vec::new()
    )]
    #[case::code_token(
        "[[stock:DEMO-A]] Demo Works:",
        NameResolution::Resolved("Demo Works"),
        RunLayout::TokenIs(TextRunKind::InlineCode),
        Vec::new()
    )]
    #[case::link_label_token(
        "[[stock:DEMO-A]] Demo Works:",
        NameResolution::Resolved("Demo Works"),
        RunLayout::TokenIs(TextRunKind::LinkLabel),
        Vec::new()
    )]
    #[case::formatted_token(
        "[[stock:DEMO-A]] Demo Works:",
        NameResolution::Resolved("Demo Works"),
        RunLayout::FormattedToken,
        Vec::new()
    )]
    #[case::formatted_candidate(
        "[[stock:DEMO-A]] **Demo Works**",
        NameResolution::Resolved("Demo Works"),
        RunLayout::FormattedCandidate,
        Vec::new()
    )]
    #[case::token_inside_url(
        "[[stock:DEMO-A]] Demo Works:",
        NameResolution::Resolved("Demo Works"),
        RunLayout::Plain,
        vec![0..16],
    )]
    #[case::candidate_inside_url(
        "[[stock:DEMO-A]] Demo Works:",
        NameResolution::Resolved("Demo Works"),
        RunLayout::Plain,
        vec![17..27],
    )]
    #[case::next_line(
        "[[stock:DEMO-A]]\nDemo Works:",
        NameResolution::Resolved("Demo Works"),
        RunLayout::Plain,
        Vec::new()
    )]
    fn skips_partial_unknown_or_non_prose_matches(
        #[case] source: &str,
        #[case] resolution: NameResolution<'_>,
        #[case] layout: RunLayout,
        #[case] url_ranges: Vec<Range<usize>>,
    ) {
        assert_eq!(
            lint_source(source, resolution, layout, url_ranges),
            Vec::new(),
        );
    }

    #[test]
    fn applies_the_fix_without_changing_lines_or_reintroducing_a_diagnostic() {
        let source = indoc::indoc! {"
            [[stock:DEMO-A]] Demo Works Demo Works:
            summary
        "};
        let edits = lint_source(
            source,
            NameResolution::Resolved("Demo Works"),
            RunLayout::Plain,
            Vec::new(),
        )
        .into_iter()
        .filter_map(|diagnostic| diagnostic.fix)
        .collect::<Vec<_>>();
        let fixed = apply_safe_edits(source, &edits).expect("the rule returns a safe edit");
        let remaining_diagnostics = lint_source(
            &fixed,
            NameResolution::Resolved("Demo Works"),
            RunLayout::Plain,
            Vec::new(),
        );

        assert_eq!(
            (
                fixed.clone(),
                remaining_diagnostics,
                fixed.matches('\n').count(),
            ),
            (
                indoc::indoc! {"
                    [[stock:DEMO-A]]:
                    summary
                "}
                .to_string(),
                Vec::new(),
                source.matches('\n').count(),
            ),
        );
    }
}
