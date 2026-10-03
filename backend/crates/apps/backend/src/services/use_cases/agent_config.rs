use std::sync::Arc;

use core_application::agent_config::AgentConfigUseCases;
use gateway_postgres::PostgresAgentConfigRepository;

use super::UseCases;

impl UseCases {
    pub fn agent_configs(&self) -> AgentConfigUseCases {
        AgentConfigUseCases::new(
            self.unit_of_work.clone(),
            Arc::new(PostgresAgentConfigRepository::new(self.db.clone())),
        )
    }
}
