use std::sync::Arc;

use core_application::account_risk_policy::AccountRiskPolicyUseCases;
use gateway_postgres::PostgresAccountRiskPolicyRepository;

use super::UseCases;

impl UseCases {
    pub fn account_risk_policies(&self) -> AccountRiskPolicyUseCases {
        AccountRiskPolicyUseCases::new(Arc::new(PostgresAccountRiskPolicyRepository::new(
            self.db.clone(),
        )))
    }
}
