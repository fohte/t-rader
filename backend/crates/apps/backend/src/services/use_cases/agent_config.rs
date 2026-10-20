use std::sync::Arc;

use core_application::agent_config::{AgentConfigRepository, AgentConfigUseCases};
use core_application::subagent_definition::{
    SubagentDefinitionUseCases, SubagentDefinitionRepository,
};
use gateway_postgres::{PostgresAgentConfigRepository, PostgresSubagentDefinitionRepository};

use super::UseCases;

impl UseCases {
    pub fn agent_configs(&self) -> AgentConfigUseCases {
        let agent_config_repository = Arc::new(PostgresAgentConfigRepository::new(self.db.clone()));
        AgentConfigUseCases::new(
            self.unit_of_work.clone(),
            agent_config_repository,
            Arc::new(PostgresSubagentDefinitionRepository::new(self.db.clone())),
            self.change_history.clone(),
        )
    }

    pub fn subagent_definitions(&self) -> SubagentDefinitionUseCases {
        let agent_config_repository: Arc<dyn AgentConfigRepository + Send + Sync> =
            Arc::new(PostgresAgentConfigRepository::new(self.db.clone()));
        let subagent_definition_repository: Arc<dyn SubagentDefinitionRepository + Send + Sync> =
            Arc::new(PostgresSubagentDefinitionRepository::new(self.db.clone()));
        SubagentDefinitionUseCases::new(
            self.unit_of_work.clone(),
            subagent_definition_repository,
            agent_config_repository,
        )
    }
}
