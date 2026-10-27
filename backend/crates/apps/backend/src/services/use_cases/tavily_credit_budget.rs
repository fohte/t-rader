use core_application::tavily_credit_budget::{
    SharedTavilyCreditBudgetUseCases, SystemTavilyCreditBudgetClock,
    TavilyCreditBudgetConfig, TavilyCreditBudgetUseCases,
};
use core_application::tavily_retrieval_cache::TavilyCredentialGeneration;
use core_application::web_search::SharedWebSearchClient;
use gateway_postgres::{PostgresTavilyCreditBudgetRepository, PostgresTavilyRetrievalCacheRepository};

use super::UseCases;

impl UseCases {
    pub fn tavily_credit_budget(
        &self,
        provider: Option<SharedWebSearchClient>,
        config: Option<TavilyCreditBudgetConfig>,
        credential_generation: Option<TavilyCredentialGeneration>,
    ) -> SharedTavilyCreditBudgetUseCases {
        std::sync::Arc::new(TavilyCreditBudgetUseCases::new(
            self.unit_of_work.clone(),
            std::sync::Arc::new(PostgresTavilyCreditBudgetRepository::new()),
            std::sync::Arc::new(PostgresTavilyRetrievalCacheRepository::new(
                self.unit_of_work.clone(),
            )),
            provider,
            config,
            credential_generation,
            std::sync::Arc::new(SystemTavilyCreditBudgetClock),
        ))
    }
}
