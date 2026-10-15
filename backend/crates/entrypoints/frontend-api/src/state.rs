use std::sync::Arc;

use core_application::account_risk_policy::AccountRiskPolicyUseCases;
use core_application::agent_config::AgentConfigUseCases;
use core_application::agent_task_client::{
    AgentTaskClient, DisabledAgentTaskClient, SharedAgentTaskClient,
};
use core_application::annotation::{AnnotationReadUseCases, AnnotationUseCases};
use core_application::bars::BarsUseCases;
use core_application::calendar::read_use_cases::CalendarEventReadUseCases;
use core_application::change_history::ChangeHistoryUseCases;
use core_application::comment::{CommentReadUseCases, CommentUseCases};
use core_application::custom_indicator::CustomIndicatorUseCases;
use core_application::financial_summary::FinancialSummaryUseCases;
use core_application::group_axis::GroupAxisUseCases;
use core_application::indicator_observation::IndicatorObservationUseCases;
use core_application::ingest_status::IngestStatusUseCase;
use core_application::kata_exec::SharedKataExecutor;
use core_application::llm_client::SharedLlmClient;
use core_application::note::{NoteReadUseCases, NoteUseCases};
use core_application::note_kind::NoteKindUseCases;
use core_application::paper_trade::PaperTradeUseCases;
use core_application::portfolio_history::PortfolioHistoryUseCases;
use core_application::prediction::PredictionUseCases;
use core_application::refs::RefUseCases;
use core_application::rss_feed::RssFeedUseCases;
use core_application::screener::ScreenerUseCases;
use core_application::stock_group::StockGroupUseCases;
use core_application::strategy::StrategyUseCases;
use core_application::strategy_earnings_target::StrategyEarningsTargetUseCases;
use core_application::strategy_scope::StrategyScopeUseCases;
use core_application::strategy_task::StrategyTaskUseCases;
use core_application::trade::{TradeNoteUseCases, TradeSummaryUseCases, TradeUseCases};
use core_application::trigger::TriggerUseCases;
use core_application::valuation::ValuationUseCases;

#[derive(Clone)]
pub struct FrontendApiState {
    pub account_risk_policy_use_cases: AccountRiskPolicyUseCases,
    pub agent_config_use_cases: AgentConfigUseCases,
    pub annotation_read_use_cases: AnnotationReadUseCases,
    pub annotation_use_cases: AnnotationUseCases,
    pub bars_use_cases: BarsUseCases,
    pub calendar_event_read_use_cases: CalendarEventReadUseCases,
    pub change_history_use_cases: ChangeHistoryUseCases,
    pub comment_read_use_cases: CommentReadUseCases,
    pub comment_use_cases: CommentUseCases,
    pub custom_indicator_use_cases: CustomIndicatorUseCases,
    pub financial_summary_use_cases: FinancialSummaryUseCases,
    pub group_axis_use_cases: GroupAxisUseCases,
    pub indicator_observation_use_cases: IndicatorObservationUseCases,
    pub ingest_status_use_case: Arc<IngestStatusUseCase>,
    pub note_kind_use_cases: NoteKindUseCases,
    pub note_read_use_cases: NoteReadUseCases,
    pub note_use_cases: NoteUseCases,
    pub paper_trade_use_cases: PaperTradeUseCases,
    pub portfolio_history_use_cases: PortfolioHistoryUseCases,
    pub prediction_use_cases: PredictionUseCases,
    pub ref_use_cases: RefUseCases,
    pub stock_group_use_cases: StockGroupUseCases,
    pub rss_feed_use_cases: RssFeedUseCases,
    pub screener_use_cases: ScreenerUseCases,
    pub strategy_scope_use_cases: Arc<StrategyScopeUseCases>,
    pub strategy_task_use_cases: StrategyTaskUseCases,
    pub strategy_earnings_target_use_cases: StrategyEarningsTargetUseCases,
    pub strategy_use_cases: StrategyUseCases,
    pub trigger_use_cases: TriggerUseCases,
    pub trade_note_use_cases: TradeNoteUseCases,
    pub trade_summary_use_cases: TradeSummaryUseCases,
    pub trade_use_cases: TradeUseCases,
    pub valuation_use_cases: ValuationUseCases,
    pub agent_task_client: SharedAgentTaskClient,
    pub kata_executor: Option<SharedKataExecutor>,
    pub llm_gateway_client: Option<SharedLlmClient>,
    pub agent_tool_summaries: Vec<(String, Option<String>)>,
}

impl FrontendApiState {
    pub fn disabled_agent_task_client() -> SharedAgentTaskClient {
        let client: Arc<dyn AgentTaskClient + Send + Sync> = Arc::new(DisabledAgentTaskClient);
        client
    }
}
