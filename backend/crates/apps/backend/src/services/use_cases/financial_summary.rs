use std::sync::Arc;

use core_application::financial_summary::FinancialSummaryUseCases;
use gateway_postgres::PostgresFinancialSummaryRepository;

use super::UseCases;

impl UseCases {
    pub fn financial_summaries(&self) -> FinancialSummaryUseCases {
        FinancialSummaryUseCases::new(
            self.unit_of_work.clone(),
            Arc::new(PostgresFinancialSummaryRepository::new(self.db.clone())),
        )
    }
}
