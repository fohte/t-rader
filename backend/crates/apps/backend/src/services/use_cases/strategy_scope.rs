use std::sync::Arc;

use core_application::strategy_scope::StrategyScopeUseCases;
use gateway_postgres::PostgresStrategyScopeSource;

use super::UseCases;

impl UseCases {
    pub fn strategy_scope(&self) -> StrategyScopeUseCases {
        StrategyScopeUseCases::new(Arc::new(PostgresStrategyScopeSource::new(
            self.db.clone(),
        )))
    }
}

#[cfg(test)]
mod tests {
    use crate::services::use_cases::build_use_cases;
    use crate::testing::insert_test_strategy;

    #[backend_test_macros::database_test]
    async fn strategy_scope_use_case_returns_scope_for_existing_strategy(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let existing_id = insert_test_strategy(&db, "scope test").await;
        let use_cases = build_use_cases(db);

        let result = use_cases
            .strategy_scope()
            .verify(existing_id)
            .await
            .map(|scope| scope.id());

        assert_eq!(result, Ok(existing_id));
    }
}
