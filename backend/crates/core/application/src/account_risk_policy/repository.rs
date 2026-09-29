use async_trait::async_trait;
use serde_json::Value;

use crate::persistence::PersistenceError;

#[async_trait]
pub trait AccountRiskPolicyRepository: Send + Sync {
    async fn find_current(&self) -> Result<Option<Value>, PersistenceError>;
    async fn save(&self, risk_policy: Value) -> Result<Value, PersistenceError>;
}

pub type SharedAccountRiskPolicyRepository =
    std::sync::Arc<dyn AccountRiskPolicyRepository + Send + Sync>;
