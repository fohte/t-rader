use core_application::tavily_credit_budget::{
    SharedTavilyCreditBudgetUseCases, SystemTavilyCreditBudgetClock,
    TavilyCreditBudgetConfig, TavilyCreditBudgetUseCases,
};
use core_application::web_search::SharedWebSearchClient;
use gateway_postgres::PostgresTavilyCreditBudgetRepository;

use super::UseCases;

impl UseCases {
    pub fn tavily_credit_budget(
        &self,
        provider: Option<SharedWebSearchClient>,
        config: Option<TavilyCreditBudgetConfig>,
    ) -> SharedTavilyCreditBudgetUseCases {
        std::sync::Arc::new(TavilyCreditBudgetUseCases::new(
            self.unit_of_work.clone(),
            std::sync::Arc::new(PostgresTavilyCreditBudgetRepository::new()),
            provider,
            config,
            std::sync::Arc::new(SystemTavilyCreditBudgetClock),
        ))
    }
}
