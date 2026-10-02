use std::sync::Arc;

use axum::extract::FromRef;
use core_application::account_risk_policy::AccountRiskPolicyUseCases;
use core_application::agent_config::AgentConfigUseCases;
use core_application::agent_task_client::{
    AgentTaskClient, DisabledAgentTaskClient, SharedAgentTaskClient,
};
use core_application::annotation::{AnnotationReadUseCases, AnnotationUseCases};
use core_application::bars::BarsUseCases;
use core_application::change_history::ChangeHistoryUseCases;
use core_application::comment::{CommentReadUseCases, CommentUseCases};
use core_application::custom_indicator::CustomIndicatorUseCases;
use core_application::group_axis::GroupAxisUseCases;
use core_application::kata_exec::SharedKataExecutor;
use core_application::llm_client::SharedLlmClient;
use core_application::note::{NoteReadUseCases, NoteUseCases};
use core_application::note_kind::NoteKindUseCases;
use core_application::prediction::PredictionUseCases;
use core_application::refs::RefUseCases;
use core_application::rss_feed::RssFeedUseCases;
use core_application::strategy::StrategyUseCases;
use core_application::strategy_scope::StrategyScopeUseCases;
use core_application::strategy_task::{StrategyTaskReconcileJobUseCases, StrategyTaskUseCases};
use core_application::trade::{TradeNoteUseCases, TradeUseCases};
use core_application::trigger::TriggerUseCases;

#[derive(Clone)]
pub struct AppState {
    pub account_risk_policy_use_cases: AccountRiskPolicyUseCases,
    pub agent_config_use_cases: AgentConfigUseCases,
    pub annotation_read_use_cases: AnnotationReadUseCases,
    pub annotation_use_cases: AnnotationUseCases,
    pub bars_use_cases: BarsUseCases,
    pub change_history_use_cases: ChangeHistoryUseCases,
    pub comment_read_use_cases: CommentReadUseCases,
    pub comment_use_cases: CommentUseCases,
    pub custom_indicator_use_cases: CustomIndicatorUseCases,
    pub group_axis_use_cases: GroupAxisUseCases,
    pub note_kind_use_cases: NoteKindUseCases,
    pub note_read_use_cases: NoteReadUseCases,
    pub note_use_cases: NoteUseCases,
    pub prediction_use_cases: PredictionUseCases,
    pub ref_use_cases: RefUseCases,
    pub rss_feed_use_cases: RssFeedUseCases,
    pub strategy_scope_use_cases: Arc<StrategyScopeUseCases>,
    pub strategy_task_use_cases: StrategyTaskUseCases,
    pub strategy_use_cases: StrategyUseCases,
    pub trigger_use_cases: TriggerUseCases,
    pub trade_note_use_cases: TradeNoteUseCases,
    pub trade_use_cases: TradeUseCases,
    pub agent_task_client: SharedAgentTaskClient,
    pub kata_executor: Option<SharedKataExecutor>,
    pub llm_gateway_client: Option<SharedLlmClient>,
    pub agent_tool_summaries: Vec<(String, Option<String>)>,
    pub agent_task_notifications: AgentTaskNotificationsState,
}

#[derive(Clone)]
pub struct AgentTaskNotificationsState {
    pub strategy_task_reconcile_job_use_cases: StrategyTaskReconcileJobUseCases,
    pub webhook_token: Arc<str>,
}

#[derive(Clone)]
pub struct ExternalHookState {
    pub trigger_use_cases: TriggerUseCases,
    pub agent_task_client: SharedAgentTaskClient,
}

impl FromRef<AppState> for AgentTaskNotificationsState {
    fn from_ref(state: &AppState) -> Self {
        state.agent_task_notifications.clone()
    }
}

impl FromRef<AppState> for ExternalHookState {
    fn from_ref(state: &AppState) -> Self {
        Self {
            trigger_use_cases: state.trigger_use_cases.clone(),
            agent_task_client: state.agent_task_client.clone(),
        }
    }
}

impl AppState {
    pub fn disabled_agent_task_client() -> SharedAgentTaskClient {
        let client: Arc<dyn AgentTaskClient + Send + Sync> = Arc::new(DisabledAgentTaskClient);
        client
    }
}
