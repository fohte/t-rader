use core_domain::markdown_lint::{RuleDefinition, rules};

pub const AGENT_NOTE_WRITE: &[&RuleDefinition] = &[
    &rules::bar_count::RULE,
    &rules::price::RULE,
    &rules::relative::RULE,
];

pub const NOTE_DISPLAY: &[&RuleDefinition] = &[];
