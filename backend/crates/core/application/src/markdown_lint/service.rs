use std::collections::{HashMap, HashSet};
use std::fmt::{Debug, Formatter};

use core_domain::markdown_lint::{
    LintError, LintReport, ParsedMarkdown, ParsedMarkdownError, ResolvedReferenceNames,
    TextRunKind, lint, requirements,
};
use core_domain::note_reference::parse_note_reference;
use thiserror::Error;

use super::markdown::parse_markdown;
use crate::refs::{RefUseCaseError, RefUseCases};

#[derive(Clone, Default)]
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
        .filter(|token| is_plain_text_token(document, &token.range))
        .filter_map(|token| document.source().get(token.range.clone()))
        .filter_map(|token| token.strip_prefix("[[")?.strip_suffix("]]"))
        .filter_map(|inner| parse_note_reference(inner).ok())
        .filter_map(|(kind, id)| (kind == "stock").then_some(id))
        .filter(|id| seen.insert(id.clone()))
        .collect()
}

fn is_plain_text_token(document: &ParsedMarkdown<'_>, token: &std::ops::Range<usize>) -> bool {
    document.runs().iter().any(|run| {
        run.kind == TextRunKind::Text
            && run.formatting_depth == 0
            && run.range.start <= token.start
            && token.end <= run.range.end
    }) && !document
        .url_ranges()
        .iter()
        .any(|url| url.start < token.end && token.start < url.end)
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Arc;

    use async_trait::async_trait;
    use core_domain::markdown_lint::{LintDiagnostic, LintReport, Severity, TextEdit};
    use rstest::rstest;
    use tokio::sync::Mutex;

    use super::{MarkdownLintService, MarkdownLintServiceError};
    use crate::markdown_lint::{AGENT_NOTE_WRITE, NOTE_DISPLAY};
    use crate::persistence::PersistenceError;
    use crate::refs::{
        IndicatorRef, RefRepository, RefRepositoryError, RefSearchMatch, RefTerm, RefUseCases,
        SharedRefRepository, StockRef,
    };
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

    struct StubRefRepository {
        names: HashMap<String, String>,
        fail_stock_lookup: bool,
        stock_requests: Mutex<Vec<Vec<String>>>,
    }

    #[async_trait]
    impl RefRepository for StubRefRepository {
        async fn list_stocks(&self, _: Option<&str>) -> Result<Vec<StockRef>, RefRepositoryError> {
            Ok(Vec::new())
        }

        async fn find_stock(&self, _: &str) -> Result<Option<StockRef>, RefRepositoryError> {
            Ok(None)
        }

        async fn stock_sectors(
            &self,
            _: &[String],
        ) -> Result<HashMap<String, Option<String>>, RefRepositoryError> {
            Ok(HashMap::new())
        }

        async fn list_indicators(
            &self,
            _: Option<&str>,
        ) -> Result<Vec<IndicatorRef>, RefRepositoryError> {
            Ok(Vec::new())
        }

        async fn find_indicator(
            &self,
            _: &str,
        ) -> Result<Option<IndicatorRef>, RefRepositoryError> {
            Ok(None)
        }

        async fn search_all(
            &self,
            _: &str,
            _: u64,
        ) -> Result<Vec<RefSearchMatch>, RefRepositoryError> {
            Ok(Vec::new())
        }

        async fn stock_names(
            &self,
            ids: &[String],
        ) -> Result<HashMap<String, String>, RefRepositoryError> {
            self.stock_requests.lock().await.push(ids.to_vec());
            if self.fail_stock_lookup {
                return Err(RefRepositoryError::Database(PersistenceError::Database(
                    "reference lookup failed".into(),
                )));
            }
            Ok(ids
                .iter()
                .filter_map(|id| self.names.get(id).map(|name| (id.clone(), name.clone())))
                .collect())
        }

        async fn indicator_names(
            &self,
            _: &[String],
        ) -> Result<HashMap<String, String>, RefRepositoryError> {
            Ok(HashMap::new())
        }

        async fn group_names(
            &self,
            _: &[String],
        ) -> Result<HashMap<String, String>, RefRepositoryError> {
            Ok(HashMap::new())
        }

        async fn list_terms(&self, _: &str) -> Result<Vec<RefTerm>, RefRepositoryError> {
            Ok(Vec::new())
        }

        async fn insert_term(
            &self,
            _: &UnitOfWorkTransaction,
            _: &str,
            _: &str,
            _: &str,
            _: &str,
        ) -> Result<bool, RefRepositoryError> {
            Ok(false)
        }

        async fn delete_term(
            &self,
            _: &UnitOfWorkTransaction,
            _: &str,
            _: &str,
            _: &str,
        ) -> Result<bool, RefRepositoryError> {
            Ok(false)
        }
    }

    fn service_with_names(
        names: &[(&str, &str)],
        fail_stock_lookup: bool,
    ) -> (MarkdownLintService, Arc<StubRefRepository>) {
        let repository = Arc::new(StubRefRepository {
            names: names
                .iter()
                .map(|(id, name)| ((*id).into(), (*name).into()))
                .collect(),
            fail_stock_lookup,
            stock_requests: Mutex::default(),
        });
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
            (result, repository.stock_requests.lock().await.clone(),),
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
            (report, repository.stock_requests.lock().await.clone()),
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
    #[case::display_profile(NOTE_DISPLAY)]
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
    async fn requires_a_resolver_when_selected_rules_need_stock_names() {
        let service = MarkdownLintService::default();
        let result = service
            .inspect("[[stock:DEMO-A]] 架空工業", AGENT_NOTE_WRITE)
            .await
            .map_err(|error| matches!(error, MarkdownLintServiceError::MissingReferenceResolver));

        assert_eq!(result, Err(true));
    }
}
