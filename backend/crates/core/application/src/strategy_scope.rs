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

pub struct StrategyScopeUseCases {
    source: SharedStrategyScopeSource,
}

impl StrategyScopeUseCases {
    pub fn new(source: SharedStrategyScopeSource) -> Self {
        Self { source }
    }

    pub async fn verify(&self, id: Uuid) -> Result<StrategyScope, StrategyScopeError> {
        StrategyScope::verify(id, self.source.as_ref()).await
    }
}

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

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;

    #[derive(Default)]
    struct FakeStrategyScopeSource {
        existing_ids: HashSet<Uuid>,
        requested_ids: Mutex<Vec<Vec<Uuid>>>,
    }

    #[async_trait]
    impl StrategyScopeSource for FakeStrategyScopeSource {
        async fn existing_ids(
            &self,
            ids: &[Uuid],
        ) -> Result<HashSet<Uuid>, StrategyScopeSourceError> {
            self.requested_ids
                .lock()
                .expect("lock requested ids")
                .push(ids.to_vec());
            Ok(self.existing_ids.clone())
        }
    }

    #[tokio::test]
    async fn verify_creates_scope_for_existing_id() {
        let id = Uuid::new_v4();
        let source = FakeStrategyScopeSource {
            existing_ids: HashSet::from([id]),
            ..FakeStrategyScopeSource::default()
        };

        assert_eq!(
            StrategyScope::verify(id, &source)
                .await
                .map(StrategyScope::id),
            Ok(id)
        );
    }

    #[tokio::test]
    async fn verify_strategy_ids_deduplicates_and_reports_missing_id() {
        let existing_id = Uuid::new_v4();
        let missing_id = Uuid::new_v4();
        let source = FakeStrategyScopeSource {
            existing_ids: HashSet::from([existing_id]),
            ..FakeStrategyScopeSource::default()
        };

        let result = verify_strategy_ids([existing_id, missing_id, missing_id], &source).await;
        let requested_ids = source
            .requested_ids
            .into_inner()
            .expect("read requested ids");

        assert_eq!(
            (result, requested_ids),
            (
                Err(StrategyScopeError::NotFound(missing_id)),
                vec![vec![existing_id, missing_id]],
            ),
        );
    }

    #[tokio::test]
    async fn verify_strategy_ids_skips_source_for_empty_input() {
        let source = FakeStrategyScopeSource::default();

        let result = verify_strategy_ids([], &source).await;
        let requested_ids = source
            .requested_ids
            .into_inner()
            .expect("read requested ids");

        assert_eq!((result, requested_ids), (Ok(()), Vec::new()));
    }
}
