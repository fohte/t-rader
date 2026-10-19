use core_domain::markdown_lint::{LintError, LintReport, ParsedMarkdownError, lint};
use thiserror::Error;

use super::markdown::parse_markdown;

#[derive(Debug, Clone, Copy, Default)]
pub struct MarkdownLintService;

#[derive(Debug, Error)]
pub enum MarkdownLintServiceError {
    #[error(transparent)]
    Parse(#[from] ParsedMarkdownError),
    #[error(transparent)]
    Lint(#[from] LintError),
}

impl MarkdownLintService {
    pub fn inspect(
        &self,
        source: &str,
        profile: &[&core_domain::markdown_lint::RuleDefinition],
    ) -> Result<LintReport, MarkdownLintServiceError> {
        let document = parse_markdown(source)?;
        Ok(lint(&document, profile, None)?)
    }
}
