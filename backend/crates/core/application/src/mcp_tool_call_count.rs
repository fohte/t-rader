use std::sync::Arc;

use async_trait::async_trait;
use thiserror::Error;

use crate::persistence::PersistenceError;

#[derive(Debug, Error)]
pub enum McpToolCallCountRepositoryError {
    #[error(transparent)]
    Database(#[from] PersistenceError),
}

#[async_trait]
pub trait McpToolCallCountRepository: Send + Sync {
    async fn increment(
        &self,
        task_execution_id: &str,
        tool_name: &str,
    ) -> Result<i32, McpToolCallCountRepositoryError>;

    async fn decrement(
        &self,
        task_execution_id: &str,
        tool_name: &str,
    ) -> Result<(), McpToolCallCountRepositoryError>;
}

pub type SharedMcpToolCallCountRepository = Arc<dyn McpToolCallCountRepository + Send + Sync>;

#[derive(Debug, Error)]
pub enum McpToolCallCountUseCaseError {
    #[error("{tool_name} call limit ({max_calls}) exceeded for this task execution")]
    CallLimitExceeded { tool_name: String, max_calls: u32 },
    #[error(transparent)]
    Repository(#[from] McpToolCallCountRepositoryError),
}

#[derive(Clone)]
pub struct McpToolCallCountUseCases {
    repository: SharedMcpToolCallCountRepository,
}

impl McpToolCallCountUseCases {
    pub fn new(repository: SharedMcpToolCallCountRepository) -> Self {
        Self { repository }
    }

    pub async fn reserve(
        &self,
        task_execution_id: &str,
        tool_name: &str,
        max_calls: u32,
    ) -> Result<(), McpToolCallCountUseCaseError> {
        let call_count = self
            .repository
            .increment(task_execution_id, tool_name)
            .await?;
        if i64::from(call_count) > i64::from(max_calls) {
            return Err(McpToolCallCountUseCaseError::CallLimitExceeded {
                tool_name: tool_name.to_string(),
                max_calls,
            });
        }
        Ok(())
    }

    pub async fn release(
        &self,
        task_execution_id: &str,
        tool_name: &str,
    ) -> Result<(), McpToolCallCountUseCaseError> {
        self.repository
            .decrement(task_execution_id, tool_name)
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};

    use super::{
        McpToolCallCountRepository, McpToolCallCountRepositoryError, McpToolCallCountUseCaseError,
        McpToolCallCountUseCases, SharedMcpToolCallCountRepository,
    };

    #[derive(Clone, Default)]
    struct FakeRepository {
        counts: Arc<Mutex<HashMap<(String, String), i32>>>,
    }

    #[async_trait::async_trait]
    impl McpToolCallCountRepository for FakeRepository {
        async fn increment(
            &self,
            task_execution_id: &str,
            tool_name: &str,
        ) -> Result<i32, McpToolCallCountRepositoryError> {
            let mut counts = self.counts.lock().expect("count lock");
            let count = counts
                .entry((task_execution_id.to_string(), tool_name.to_string()))
                .or_default();
            *count += 1;
            Ok(*count)
        }

        async fn decrement(
            &self,
            task_execution_id: &str,
            tool_name: &str,
        ) -> Result<(), McpToolCallCountRepositoryError> {
            let mut counts = self.counts.lock().expect("count lock");
            let count = counts
                .entry((task_execution_id.to_string(), tool_name.to_string()))
                .or_default();
            *count -= 1;
            Ok(())
        }
    }

    #[tokio::test]
    async fn reserve_increments_before_reporting_a_limit_exceeded() {
        let repository = FakeRepository::default();
        let repository_for_use_case: SharedMcpToolCallCountRepository =
            Arc::new(repository.clone());
        let use_cases = McpToolCallCountUseCases::new(repository_for_use_case);
        let first = use_cases.reserve("fictional-task", "search_web", 1).await;
        let second = use_cases.reserve("fictional-task", "search_web", 1).await;
        let count = repository
            .counts
            .lock()
            .expect("count lock")
            .get(&("fictional-task".to_string(), "search_web".to_string()))
            .copied();

        assert_eq!(
            (
                first.map_err(|error| error.to_string()),
                second.map_err(|error| error.to_string()),
                count,
            ),
            (
                Ok(()),
                Err(McpToolCallCountUseCaseError::CallLimitExceeded {
                    tool_name: "search_web".to_string(),
                    max_calls: 1,
                }
                .to_string()),
                Some(2),
            ),
        );
    }
}
