use super::super::market_numeric::{MarketNumericKind, findings_of_kind};
use super::super::{MarkdownLintContext, RuleDefinition, RuleFinding, RuleRequirements, Severity};

pub static RULE: RuleDefinition = RuleDefinition::new(
    "bar-count",
    Severity::Warning,
    RuleRequirements::MARKET_NUMERIC,
    check,
);

fn check(context: &MarkdownLintContext<'_>) -> Vec<RuleFinding> {
    findings_of_kind(context, MarketNumericKind::BarCount)
}
