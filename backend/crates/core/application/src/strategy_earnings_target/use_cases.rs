use crate::refs::{RefUseCases, ResolvedRef};
use crate::strategy_scope::StrategyScope;
use crate::unit_of_work::SharedUnitOfWork;

use super::error::StrategyEarningsTargetUseCaseError;
use super::repository::SharedStrategyEarningsTargetRepository;
use super::types::StrategyEarningsTarget;

#[derive(Clone)]
pub struct StrategyEarningsTargetUseCases {
    unit_of_work: SharedUnitOfWork,
    repository: SharedStrategyEarningsTargetRepository,
    refs: RefUseCases,
}

impl StrategyEarningsTargetUseCases {
    pub fn new(
        unit_of_work: SharedUnitOfWork,
        repository: SharedStrategyEarningsTargetRepository,
        refs: RefUseCases,
    ) -> Self {
        Self {
            unit_of_work,
            repository,
            refs,
        }
    }

    pub async fn add(
        &self,
        scope: StrategyScope,
        ref_kind: &str,
        ref_id: &str,
    ) -> Result<bool, StrategyEarningsTargetUseCaseError> {
        let reference = self.resolve_target(ref_kind, ref_id, true).await?;
        let transaction = self.unit_of_work.begin().await?;
        let changed = self
            .repository
            .insert(&transaction, scope.id(), &reference.kind, &reference.id)
            .await?;
        self.unit_of_work.commit(transaction).await?;
        Ok(changed)
    }

    pub async fn remove(
        &self,
        scope: StrategyScope,
        ref_kind: &str,
        ref_id: &str,
    ) -> Result<bool, StrategyEarningsTargetUseCaseError> {
        let reference = self.resolve_target(ref_kind, ref_id, false).await?;
        let transaction = self.unit_of_work.begin().await?;
        let changed = self
            .repository
            .delete(&transaction, scope.id(), &reference.kind, &reference.id)
            .await?;
        self.unit_of_work.commit(transaction).await?;
        Ok(changed)
    }

    pub async fn list(
        &self,
        scope: StrategyScope,
    ) -> Result<Vec<StrategyEarningsTarget>, StrategyEarningsTargetUseCaseError> {
        self.repository.list(scope.id()).await.map_err(Into::into)
    }

    async fn resolve_target(
        &self,
        ref_kind: &str,
        ref_id: &str,
        require_existing: bool,
    ) -> Result<ResolvedRef, StrategyEarningsTargetUseCaseError> {
        let ref_kind = ref_kind.trim();
        if !matches!(ref_kind, "stock" | "group") {
            return Err(StrategyEarningsTargetUseCaseError::Validation(
                "ref_kind must be stock or group".into(),
            ));
        }

        let ref_id = ref_id.trim();
        let requested = [(ref_kind.to_owned(), ref_id.to_owned())];
        let mut resolved = self.refs.resolve(&requested).await?;
        let Some(reference) = resolved.pop() else {
            return Err(StrategyEarningsTargetUseCaseError::Validation(
                "reference could not be resolved".into(),
            ));
        };
        if require_existing && reference.name.is_none() {
            return Err(StrategyEarningsTargetUseCaseError::ReferenceNotFound {
                ref_kind: reference.kind,
                ref_id: reference.id,
            });
        }
        Ok(reference)
    }
}
