use std::collections::{HashMap, HashSet};
use std::fmt::{Debug, Formatter};

use core_domain::markdown_lint::{
    LintError, LintReport, ParsedMarkdown, ParsedMarkdownError, ResolvedReferenceNames, lint,
    requirements,
};
use core_domain::note_reference::parse_note_reference;
use thiserror::Error;

use super::markdown::parse_markdown;
use crate::refs::{RefUseCaseError, RefUseCases};

#[cfg_attr(test, derive(Default))]
#[derive(Clone)]
pub struct MarkdownLintService {
    refs: Option<RefUseCases>,
}

impl Debug for MarkdownLintService {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("MarkdownLintService")
            .field("has_reference_resolver", &self.refs.is_some())
            .finish()
    }
}

#[derive(Debug, Error)]
pub enum MarkdownLintServiceError {
    #[error(transparent)]
    Parse(#[from] ParsedMarkdownError),
    #[error(transparent)]
    Lint(#[from] LintError),
    #[error("markdown lint requires a reference resolver")]
    MissingReferenceResolver,
    #[error("reference resolver returned {actual} names for {expected} requested stocks")]
    IncompleteReferenceResolution { expected: usize, actual: usize },
    #[error(transparent)]
    ReferenceResolution(#[from] RefUseCaseError),
}

impl MarkdownLintService {
    pub fn new(refs: RefUseCases) -> Self {
        Self { refs: Some(refs) }
    }

    pub async fn inspect(
        &self,
        source: &str,
        profile: &[&core_domain::markdown_lint::RuleDefinition],
    ) -> Result<LintReport, MarkdownLintServiceError> {
        let document = parse_markdown(source)?;
        let required = requirements(profile);
        let resolved_reference_names = if required.resolved_reference_names {
            let stock_ids = stock_ids(&document);
            if stock_ids.is_empty() {
                Some(HashMap::new())
            } else {
                let refs = self
                    .refs
                    .as_ref()
                    .ok_or(MarkdownLintServiceError::MissingReferenceResolver)?;
                let requested = stock_ids
                    .iter()
                    .map(|id| ("stock".to_string(), id.clone()))
                    .collect::<Vec<_>>();
                let resolved = refs.resolve(&requested).await?;
                if resolved.len() != requested.len() {
                    return Err(MarkdownLintServiceError::IncompleteReferenceResolution {
                        expected: requested.len(),
                        actual: resolved.len(),
                    });
                }
                Some(
                    requested
                        .iter()
                        .zip(resolved)
                        .map(|((kind, id), reference)| (format!("{kind}:{id}"), reference.name))
                        .collect::<ResolvedReferenceNames>(),
                )
            }
        } else {
            None
        };
        Ok(lint(&document, profile, resolved_reference_names.as_ref())?)
    }
}

fn stock_ids(document: &ParsedMarkdown<'_>) -> Vec<String> {
    let mut seen = HashSet::new();
    document
        .validated_tokens()
        .iter()
        .filter(|token| document.plain_text_run_containing(&token.range).is_some())
        .filter_map(|token| document.source().get(token.range.clone()))
        .filter_map(|token| token.strip_prefix("[[")?.strip_suffix("]]"))
        .filter_map(|inner| parse_note_reference(inner).ok())
        .filter_map(|(kind, id)| (kind == "stock").then_some(id))
        .filter(|id| seen.insert(id.clone()))
        .collect()
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use async_trait::async_trait;
    use core_domain::markdown_lint::{LintDiagnostic, LintReport, Severity, TextEdit};
    use rstest::rstest;

    use super::{MarkdownLintService, MarkdownLintServiceError};
    use crate::markdown_lint::{AGENT_NOTE_WRITE, NOTE_DISPLAY};
    use crate::refs::{RefUseCases, SharedRefRepository, test_support::FakeRefRepository};
    use crate::unit_of_work::{UnitOfWork, UnitOfWorkError, UnitOfWorkTransaction};

    struct TestUnitOfWork;

    #[async_trait]
    impl UnitOfWork for TestUnitOfWork {
        async fn begin(&self) -> Result<UnitOfWorkTransaction, UnitOfWorkError> {
            Ok(UnitOfWorkTransaction::new(()))
        }

        async fn commit(&self, _: UnitOfWorkTransaction) -> Result<(), UnitOfWorkError> {
            Ok(())
        }
    }

    fn service_with_names(
        names: &[(&str, &str)],
        fail_stock_lookup: bool,
    ) -> (MarkdownLintService, Arc<FakeRefRepository>) {
        let repository = Arc::new(FakeRefRepository::new(
            names
                .iter()
                .map(|(id, name)| ((*id).into(), (*name).into()))
                .collect(),
            fail_stock_lookup,
        ));
        let shared_repository: SharedRefRepository = repository.clone();
        let refs = RefUseCases::new(Arc::new(TestUnitOfWork), shared_repository);
        (MarkdownLintService::new(refs), repository)
    }

    #[tokio::test]
    async fn resolves_each_stock_once_and_returns_complete_diagnostics() {
        let (service, repository) = service_with_names(&[("DEMO-A", "架空工業")], false);
        let source = indoc::indoc! {"
            [[stock:DEMO-A]] 架空工業
            [[stock:DEMO-A]] 架空工業
        "};
        let result = service
            .inspect(source, AGENT_NOTE_WRITE)
            .await
            .expect("reference names are available");

        assert_eq!(
            (result, repository.stock_requests().await,),
            (
                LintReport {
                    diagnostics: vec![
                        LintDiagnostic {
                            rule_id: "duplicate-stock-name",
                            id: "duplicate-stock-name:stock%3ADEMO-A:1".into(),
                            severity: Severity::Error,
                            range: 17..29,
                            message: "銘柄リンク直後の同名を削除してください。".into(),
                            fix: Some(TextEdit {
                                range: 16..29,
                                replacement: String::new(),
                            }),
                        },
                        LintDiagnostic {
                            rule_id: "duplicate-stock-name",
                            id: "duplicate-stock-name:stock%3ADEMO-A:2".into(),
                            severity: Severity::Error,
                            range: 47..59,
                            message: "銘柄リンク直後の同名を削除してください。".into(),
                            fix: Some(TextEdit {
                                range: 46..59,
                                replacement: String::new(),
                            }),
                        },
                    ],
                },
                vec![vec!["DEMO-A".into()]],
            ),
        );
    }

    #[tokio::test]
    async fn unresolved_stock_names_produce_no_inferred_diagnostics() {
        let (service, repository) = service_with_names(&[], false);
        let report = service
            .inspect("[[stock:DEMO-A]] 架空工業", AGENT_NOTE_WRITE)
            .await
            .expect("an unresolved name is a valid resolver result");

        assert_eq!(
            (report, repository.stock_requests().await),
            (LintReport::default(), vec![vec!["DEMO-A".into()]]),
        );
    }

    #[tokio::test]
    async fn returns_resolver_failures_instead_of_treating_them_as_unresolved_names() {
        let (service, _) = service_with_names(&[], true);
        let result = service
            .inspect("[[stock:DEMO-A]] 架空工業", AGENT_NOTE_WRITE)
            .await
            .map_err(|error| matches!(error, MarkdownLintServiceError::ReferenceResolution(_)));

        assert_eq!(result, Err(true));
    }

    #[rstest]
    #[case::empty_profile(&[])]
    #[tokio::test]
    async fn does_not_resolve_stocks_for_profiles_without_reference_name_requirements(
        #[case] profile: &[&core_domain::markdown_lint::RuleDefinition],
    ) {
        let service = MarkdownLintService::default();
        let result = service
            .inspect("[[stock:DEMO-A]] 架空工業", profile)
            .await
            .map_err(|error| matches!(error, MarkdownLintServiceError::MissingReferenceResolver));

        assert_eq!(result, Ok(LintReport::default()));
    }

    #[tokio::test]
    async fn note_display_resolves_names_for_duplicate_diagnostics() {
        let (service, repository) = service_with_names(&[("DEMO-A", "架空工業")], false);
        let result = service
            .inspect("[[stock:DEMO-A]] 架空工業", NOTE_DISPLAY)
            .await
            .expect("reference names are available");

        assert_eq!(
            (result, repository.stock_requests().await),
            (
                LintReport {
                    diagnostics: vec![LintDiagnostic {
                        rule_id: "duplicate-stock-name",
                        id: "duplicate-stock-name:stock%3ADEMO-A:1".into(),
                        severity: Severity::Error,
                        range: 17..29,
                        message: "銘柄リンク直後の同名を削除してください。".into(),
                        fix: Some(TextEdit {
                            range: 16..29,
                            replacement: String::new(),
                        }),
                    }],
                },
                vec![vec!["DEMO-A".into()]],
            ),
        );
    }

    #[tokio::test]
    async fn requires_a_resolver_when_selected_rules_need_stock_names() {
        let service = MarkdownLintService::default();
        let result = service
            .inspect("[[stock:DEMO-A]] 架空工業", AGENT_NOTE_WRITE)
            .await
            .map_err(|error| matches!(error, MarkdownLintServiceError::MissingReferenceResolver));

        assert_eq!(result, Err(true));
    }
}
