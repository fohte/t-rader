use core_application::agent_task_client::SharedAgentTaskClient;
use core_application::trigger::TriggerUseCases;

#[derive(Clone)]
pub struct ExternalWebhookState {
    pub trigger_use_cases: TriggerUseCases,
    pub agent_task_client: SharedAgentTaskClient,
}
