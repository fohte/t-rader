use std::ops::Range;

mod decision;
mod document;
mod fix;
mod id;
pub mod market_numeric;
pub mod normalization;
pub mod rules;
mod runner;
mod types;

pub use decision::{
    AcceptedDisposition, Disposition, DispositionEvaluation, evaluate_dispositions,
};
pub use document::{ParsedMarkdown, ParsedMarkdownError, TextRun, TextRunKind, ValidatedTokenSpan};
pub use fix::{FixError, apply_safe_edits};
pub(crate) use id::lint_id;
pub use id::warning_id_group;
pub use runner::{LintError, lint, requirements};
pub use types::{
    LintDiagnostic, LintReport, MarkdownLintContext, ResolvedReferenceNames, RuleDefinition,
    RuleFinding, RuleRequirements, Severity, TextEdit,
};

pub(crate) fn ranges_overlap(left: &Range<usize>, right: &Range<usize>) -> bool {
    left.start < right.end && right.start < left.end
}
