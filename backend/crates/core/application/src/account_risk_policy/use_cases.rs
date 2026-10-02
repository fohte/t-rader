use serde_json::Value;

use super::error::AccountRiskPolicyRepositoryError;
use super::repository::SharedAccountRiskPolicyRepository;

#[derive(Clone)]
pub struct AccountRiskPolicyUseCases {
    repository: SharedAccountRiskPolicyRepository,
}

impl AccountRiskPolicyUseCases {
    pub fn new(repository: SharedAccountRiskPolicyRepository) -> Self {
        Self { repository }
    }

    pub async fn find_current(&self) -> Result<Option<Value>, AccountRiskPolicyRepositoryError> {
        self.repository.find_current().await
    }

    pub async fn save(
        &self,
        risk_policy: Value,
    ) -> Result<Value, AccountRiskPolicyRepositoryError> {
        self.repository.save(risk_policy).await
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use async_trait::async_trait;
    use serde_json::{Value, json};
    use tokio::sync::Mutex;

    use crate::account_risk_policy::{
        AccountRiskPolicyRepository, AccountRiskPolicyRepositoryError, AccountRiskPolicyUseCases,
    };

    #[derive(Default)]
    struct FakeAccountRiskPolicyRepository {
        current: Mutex<Option<Value>>,
    }

    #[async_trait]
    impl AccountRiskPolicyRepository for FakeAccountRiskPolicyRepository {
        async fn find_current(&self) -> Result<Option<Value>, AccountRiskPolicyRepositoryError> {
            Ok(self.current.lock().await.clone())
        }

        async fn save(
            &self,
            risk_policy: Value,
        ) -> Result<Value, AccountRiskPolicyRepositoryError> {
            *self.current.lock().await = Some(risk_policy.clone());
            Ok(risk_policy)
        }
    }

    #[tokio::test]
    async fn find_current_returns_none_when_policy_is_unset() {
        let use_cases =
            AccountRiskPolicyUseCases::new(Arc::new(FakeAccountRiskPolicyRepository::default()));

        assert_eq!(use_cases.find_current().await.expect("read policy"), None);
    }

    #[tokio::test]
    async fn save_stores_and_returns_policy() {
        let use_cases =
            AccountRiskPolicyUseCases::new(Arc::new(FakeAccountRiskPolicyRepository::default()));
        let policy = json!({ "max_group_ratios": [] });

        let saved = use_cases.save(policy.clone()).await.expect("save policy");
        let current = use_cases.find_current().await.expect("read saved policy");

        assert_eq!((saved, current), (policy.clone(), Some(policy)));
    }
}
