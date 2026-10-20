use thiserror::Error;

use super::document::{ParsedMarkdown, ParsedMarkdownError, validate_range};
use super::id::LintIdOccurrences;
use super::market_numeric::scan_market_numeric_source;
use super::types::{
    LintDiagnostic, LintReport, MarkdownLintContext, ResolvedReferenceNames, RuleDefinition,
    RuleFinding, RuleRequirements,
};

#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum LintError {
    #[error(transparent)]
    InvalidRange(#[from] ParsedMarkdownError),
    #[error("selected rules require resolved reference names")]
    MissingResolvedReferenceNames,
}

pub fn requirements(rules: &[&RuleDefinition]) -> RuleRequirements {
    rules
        .iter()
        .fold(RuleRequirements::NONE, |mut aggregate, rule| {
            aggregate.market_numeric_analysis |= rule.requirements.market_numeric_analysis;
            aggregate.resolved_reference_names |= rule.requirements.resolved_reference_names;
            aggregate
        })
}

pub fn lint(
    document: &ParsedMarkdown<'_>,
    rules: &[&RuleDefinition],
    resolved_reference_names: Option<&ResolvedReferenceNames>,
) -> Result<LintReport, LintError> {
    let required = requirements(rules);
    if required.resolved_reference_names && resolved_reference_names.is_none() {
        return Err(LintError::MissingResolvedReferenceNames);
    }

    let market_numeric_findings = if required.market_numeric_analysis {
        scan_market_numeric_source(document.source())
    } else {
        Vec::new()
    };
    let context = MarkdownLintContext {
        document,
        market_numeric_findings: &market_numeric_findings,
        resolved_reference_names,
    };
    let mut occurrences = LintIdOccurrences::default();
    let mut diagnostics = Vec::new();

    for rule in rules {
        let mut findings = (rule.check)(&context);
        findings.sort_by_key(|finding| finding.range.start);
        for finding in findings {
            diagnostics.push(make_diagnostic(
                document.source(),
                rule,
                finding,
                &mut occurrences,
            )?);
        }
    }

    diagnostics.sort_by_key(|diagnostic| diagnostic.range.start);
    Ok(LintReport { diagnostics })
}

fn make_diagnostic(
    source: &str,
    rule: &RuleDefinition,
    finding: RuleFinding,
    occurrences: &mut LintIdOccurrences,
) -> Result<LintDiagnostic, LintError> {
    validate_range(source, &finding.range)?;
    if let Some(fix) = finding.fix.as_ref() {
        validate_range(source, &fix.range)?;
    }
    Ok(LintDiagnostic {
        rule_id: rule.id,
        id: occurrences.next(rule.id, &finding.key),
        severity: rule.severity,
        range: finding.range,
        message: finding.message,
        fix: finding.fix,
    })
}

#[cfg(test)]
mod tests {
    use std::ops::Range;

    use rstest::rstest;

    use super::{LintError, lint};
    use crate::markdown_lint::{
        MarkdownLintContext, ParsedMarkdown, RuleDefinition, RuleFinding, RuleRequirements,
        Severity, TextEdit, TextRun, ValidatedTokenSpan,
    };

    fn document(source: &str) -> ParsedMarkdown<'_> {
        ParsedMarkdown::new(source, Vec::new(), Vec::new(), Vec::new())
            .expect("synthetic ranges are valid")
    }

    #[test]
    fn keeps_numeric_diagnostics_and_absolute_ranges() {
        let source = "close 12,347円 and -6.3%";
        let document = document(source);
        let report = lint(
            &document,
            &[
                &super::super::rules::bar_count::RULE,
                &super::super::rules::price::RULE,
                &super::super::rules::relative::RULE,
            ],
            None,
        )
        .expect("all numeric rule requirements are available");

        assert_eq!(
            report.diagnostics,
            vec![
                crate::markdown_lint::LintDiagnostic {
                    rule_id: "price",
                    id: "price:12%2C347%E5%86%86:1".into(),
                    severity: Severity::Warning,
                    range: 6..15,
                    message: concat!(
                        "価格候補の数値「12,347円」がリンク外にあります。株価であれば、銘柄・日付・項目を確認して ",
                        "`[[price:<id>@<date>:<field>]]` で参照してください。株価ではない値ならそのまま残し、必要な理由を ",
                        "`warning_dispositions` に記載して警告を無視してください。"
                    )
                    .into(),
                    fix: None,
                },
                crate::markdown_lint::LintDiagnostic {
                    rule_id: "relative",
                    id: "relative:-6.3%25:1".into(),
                    severity: Severity::Warning,
                    range: 20..25,
                    message: concat!(
                        "相対表現「-6.3%」があります。株価の騰落率であれば、計算結果を手入力せず、対象期間の値を ",
                        "`[[change:<id>@<start>..<end>:<field>]]` で示してください。株価の騰落率ではない値はそのまま残し、必要な理由を ",
                        "`warning_dispositions` に記載して警告を無視してください。"
                    )
                    .into(),
                    fix: None,
                },
            ],
        );
    }

    #[test]
    fn retains_absolute_ranges_after_masked_markdown_segments() {
        let source = "注記 [[stock:sample-code]] and `ignore 98,765円` then 12,347円";
        let start = source.find("12,347円").expect("price is in the source");
        let document = document(source);
        let report = lint(
            &document,
            &[
                &super::super::rules::bar_count::RULE,
                &super::super::rules::price::RULE,
                &super::super::rules::relative::RULE,
            ],
            None,
        )
        .expect("all numeric rule requirements are available");

        assert_eq!(
            report.diagnostics,
            vec![crate::markdown_lint::LintDiagnostic {
                rule_id: "price",
                id: "price:12%2C347%E5%86%86:1".into(),
                severity: Severity::Warning,
                range: start..start + "12,347円".len(),
                message: concat!(
                    "価格候補の数値「12,347円」がリンク外にあります。株価であれば、銘柄・日付・項目を確認して ",
                    "`[[price:<id>@<date>:<field>]]` で参照してください。株価ではない値ならそのまま残し、必要な理由を ",
                    "`warning_dispositions` に記載して警告を無視してください。"
                )
                .into(),
                fix: None,
            }],
        );
    }

    #[rstest]
    #[case::valid_range(false)]
    #[case::invalid_range(true)]
    fn rejects_rule_findings_outside_the_source(#[case] invalid: bool) {
        fn valid_check(_: &MarkdownLintContext<'_>) -> Vec<RuleFinding> {
            vec![RuleFinding {
                key: "sample-key".into(),
                range: 0..1,
                message: "sample finding".into(),
                fix: None,
            }]
        }
        fn invalid_check(_: &MarkdownLintContext<'_>) -> Vec<RuleFinding> {
            vec![RuleFinding {
                key: "sample-key".into(),
                range: 0..99,
                message: "sample finding".into(),
                fix: None,
            }]
        }

        let rule = RuleDefinition::new(
            "sample-rule",
            Severity::Error,
            RuleRequirements::NONE,
            if invalid { invalid_check } else { valid_check },
        );
        let document = document("x");

        assert_eq!(
            lint(&document, &[&rule], None)
                .map(|report| report.diagnostics.len())
                .map_err(|error| matches!(error, LintError::InvalidRange(_))),
            if invalid { Err(true) } else { Ok(1) },
        );
    }

    #[test]
    fn rejects_selected_rules_when_required_facts_are_missing() {
        fn needs_names(_: &MarkdownLintContext<'_>) -> Vec<RuleFinding> {
            Vec::new()
        }

        let rule = RuleDefinition::new(
            "sample-rule",
            Severity::Error,
            RuleRequirements {
                resolved_reference_names: true,
                ..RuleRequirements::NONE
            },
            needs_names,
        );
        let document = document("sample");

        assert_eq!(
            lint(&document, &[&rule], None),
            Err(LintError::MissingResolvedReferenceNames),
        );
    }

    #[test]
    fn preserves_registry_order_for_findings_at_the_same_offset() {
        fn first(_: &MarkdownLintContext<'_>) -> Vec<RuleFinding> {
            vec![finding(0..1, "first")]
        }
        fn second(_: &MarkdownLintContext<'_>) -> Vec<RuleFinding> {
            vec![finding(0..1, "second")]
        }
        fn finding(range: Range<usize>, key: &str) -> RuleFinding {
            RuleFinding {
                key: key.into(),
                range,
                message: "sample finding".into(),
                fix: Some(TextEdit {
                    range: 0..1,
                    replacement: String::new(),
                }),
            }
        }

        let first_rule = RuleDefinition::new(
            "first-rule",
            Severity::Warning,
            RuleRequirements::NONE,
            first,
        );
        let second_rule = RuleDefinition::new(
            "second-rule",
            Severity::Warning,
            RuleRequirements::NONE,
            second,
        );
        let document = ParsedMarkdown::new(
            "x",
            vec![TextRun {
                range: 0..1,
                kind: crate::markdown_lint::TextRunKind::Text,
                quote_depth: 0,
                table_cell: None,
                formatting_depth: 0,
            }],
            vec![ValidatedTokenSpan { range: 0..1 }],
            std::iter::once(0..1).collect(),
        )
        .expect("synthetic source ranges are valid");

        assert_eq!(
            lint(&document, &[&first_rule, &second_rule], None)
                .expect("no facts are required")
                .diagnostics
                .into_iter()
                .map(|diagnostic| diagnostic.rule_id)
                .collect::<Vec<_>>(),
            vec!["first-rule", "second-rule"],
        );
    }
}
