use std::{collections::HashSet, sync::Arc};

use async_trait::async_trait;
use uuid::Uuid;

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum StrategyScopeSourceError {
    #[error("strategy scope query failed: {0}")]
    QueryFailed(String),
}

#[async_trait]
pub trait StrategyScopeSource: Send + Sync {
    async fn existing_ids(&self, ids: &[Uuid]) -> Result<HashSet<Uuid>, StrategyScopeSourceError>;
}

pub type SharedStrategyScopeSource = Arc<dyn StrategyScopeSource>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StrategyScope {
    id: Uuid,
}

impl StrategyScope {
    /// 戦略の存在を確認して、検証済みのスコープを作る。
    pub async fn verify<S: StrategyScopeSource + ?Sized>(
        id: Uuid,
        source: &S,
    ) -> Result<Self, StrategyScopeError> {
        verify_strategy_ids([id], source).await?;
        Ok(Self { id })
    }

    pub fn id(self) -> Uuid {
        self.id
    }
}

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum StrategyScopeError {
    #[error("strategy {0} not found")]
    NotFound(Uuid),
    #[error("strategy scope source failed: {0}")]
    Source(#[from] StrategyScopeSourceError),
}

/// 指定された戦略 ID の存在を一括確認する。空入力では source を呼び出さない。
pub async fn verify_strategy_ids<I, S>(ids: I, source: &S) -> Result<(), StrategyScopeError>
where
    I: IntoIterator<Item = Uuid>,
    S: StrategyScopeSource + ?Sized,
{
    let mut unique_ids = Vec::new();
    let mut seen_ids = HashSet::new();
    for id in ids {
        if seen_ids.insert(id) {
            unique_ids.push(id);
        }
    }

    if unique_ids.is_empty() {
        return Ok(());
    }

    let existing_ids = source.existing_ids(&unique_ids).await?;
    if let Some(missing_id) = unique_ids.into_iter().find(|id| !existing_ids.contains(id)) {
        return Err(StrategyScopeError::NotFound(missing_id));
    }

    Ok(())
}

/// テストでのみ、存在確認済みの `StrategyScope` を作るために使う。
#[cfg(feature = "test-support")]
impl From<Uuid> for StrategyScope {
    fn from(id: Uuid) -> Self {
        Self { id }
    }
}
