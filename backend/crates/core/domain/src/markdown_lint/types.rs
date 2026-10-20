use std::collections::HashMap;
use std::ops::Range;

use serde::Serialize;

use super::document::ParsedMarkdown;
use super::market_numeric::MarketNumericFinding;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Warning,
    Error,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RuleRequirements {
    pub market_numeric_analysis: bool,
    pub resolved_reference_names: bool,
}

impl RuleRequirements {
    pub const NONE: Self = Self {
        market_numeric_analysis: false,
        resolved_reference_names: false,
    };

    pub const MARKET_NUMERIC: Self = Self {
        market_numeric_analysis: true,
        resolved_reference_names: false,
    };
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleFinding {
    pub key: String,
    pub range: Range<usize>,
    pub message: String,
    pub fix: Option<TextEdit>,
}

impl RuleFinding {
    pub(crate) fn from_market_numeric(finding: &MarketNumericFinding) -> Self {
        Self {
            key: finding.key.clone(),
            range: finding.range.clone(),
            message: finding.message.clone(),
            fix: None,
        }
    }
}

pub struct RuleDefinition {
    pub(super) id: &'static str,
    pub(super) severity: Severity,
    pub(super) requirements: RuleRequirements,
    pub(super) check: for<'a> fn(&MarkdownLintContext<'a>) -> Vec<RuleFinding>,
}

impl RuleDefinition {
    pub(super) const fn new(
        id: &'static str,
        severity: Severity,
        requirements: RuleRequirements,
        check: for<'a> fn(&MarkdownLintContext<'a>) -> Vec<RuleFinding>,
    ) -> Self {
        Self {
            id,
            severity,
            requirements,
            check,
        }
    }
}

pub struct MarkdownLintContext<'a> {
    pub document: &'a ParsedMarkdown<'a>,
    pub market_numeric_findings: &'a [MarketNumericFinding],
    pub resolved_reference_names: Option<&'a ResolvedReferenceNames>,
}

pub type ResolvedReferenceNames = HashMap<String, Option<String>>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TextEdit {
    pub range: Range<usize>,
    pub replacement: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LintDiagnostic {
    pub rule_id: &'static str,
    pub id: String,
    pub severity: Severity,
    pub range: Range<usize>,
    pub message: String,
    pub fix: Option<TextEdit>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct LintReport {
    pub diagnostics: Vec<LintDiagnostic>,
}
