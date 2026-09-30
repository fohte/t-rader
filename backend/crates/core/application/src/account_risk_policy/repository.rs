use async_trait::async_trait;
use serde_json::Value;

use super::error::AccountRiskPolicyRepositoryError;

#[async_trait]
pub trait AccountRiskPolicyRepository: Send + Sync {
    async fn find_current(&self) -> Result<Option<Value>, AccountRiskPolicyRepositoryError>;
    async fn save(&self, risk_policy: Value) -> Result<Value, AccountRiskPolicyRepositoryError>;
}

pub type SharedAccountRiskPolicyRepository =
    std::sync::Arc<dyn AccountRiskPolicyRepository + Send + Sync>;
