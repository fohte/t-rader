use super::super::market_numeric::MarketNumericKind;
use super::super::{MarkdownLintContext, RuleDefinition, RuleFinding, RuleRequirements, Severity};

pub static RULE: RuleDefinition = RuleDefinition::new(
    "bar-count",
    Severity::Warning,
    RuleRequirements::MARKET_NUMERIC,
    check,
);

fn check(context: &MarkdownLintContext<'_>) -> Vec<RuleFinding> {
    context
        .market_numeric_findings
        .iter()
        .filter(|finding| finding.kind == MarketNumericKind::BarCount)
        .map(RuleFinding::from_market_numeric)
        .collect()
}
