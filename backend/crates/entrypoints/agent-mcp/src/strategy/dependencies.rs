use std::sync::Arc;

use core_application::account_risk_policy::AccountRiskPolicyUseCases;
use core_application::annotation::{AnnotationReadUseCases, AnnotationUseCases};
use core_application::bars::BarsUseCases;
use core_application::comment::{CommentReadUseCases, CommentUseCases};
use core_application::custom_indicator::CustomIndicatorUseCases;
use core_application::daily_bar_source::SharedDailyBarSource;
use core_application::financial_summary::FinancialSummaryUseCases;
use core_application::indicator_observation::IndicatorObservationUseCases;
use core_application::kata_exec::SharedKataExecutor;
use core_application::llm_client::SharedLlmClient;
use core_application::margin::MarginUseCases;
use core_application::mcp_tool_call_count::McpToolCallCountUseCases;
use core_application::news::NewsUseCases;
use core_application::note::{NoteReadUseCases, NoteUseCases};
use core_application::note_kind::NoteKindUseCases;
use core_application::prediction::PredictionUseCases;
use core_application::refs::RefUseCases;
use core_application::shareholding_structure::ShareholdingStructureUseCases;
use core_application::short_ratio::ShortRatioUseCases;
use core_application::short_sale_report::ShortSaleReportUseCases;
use core_application::stock_group::StockGroupUseCases;
use core_application::strategy::StrategyUseCases;
use core_application::strategy_scope::StrategyScopeUseCases;
use core_application::strategy_task_step_evidence::StrategyTaskStepEvidenceUseCases;
use core_application::trade::TradeUseCases;
use core_application::valuation::ValuationUseCases;

#[derive(Clone)]
pub struct StrategyServerDependencies {
    pub account_risk_policies: AccountRiskPolicyUseCases,
    pub annotation_reads: AnnotationReadUseCases,
    pub annotations: AnnotationUseCases,
    pub bars: BarsUseCases,
    pub comment_reads: CommentReadUseCases,
    pub comments: CommentUseCases,
    pub custom_indicators: CustomIndicatorUseCases,
    pub daily_bar_source: Option<SharedDailyBarSource>,
    pub financial_summaries: FinancialSummaryUseCases,
    pub indicator_observations: IndicatorObservationUseCases,
    pub kata_executor: Option<SharedKataExecutor>,
    pub llm_client: Option<SharedLlmClient>,
    pub margins: MarginUseCases,
    pub mcp_tool_call_counts: McpToolCallCountUseCases,
    pub news: NewsUseCases,
    pub note_kinds: NoteKindUseCases,
    pub note_reads: NoteReadUseCases,
    pub notes: NoteUseCases,
    pub predictions: PredictionUseCases,
    pub refs: RefUseCases,
    pub shareholding_structures: ShareholdingStructureUseCases,
    pub short_ratios: ShortRatioUseCases,
    pub short_sale_reports: ShortSaleReportUseCases,
    pub stock_groups: StockGroupUseCases,
    pub strategies: StrategyUseCases,
    pub strategy_scope: Arc<StrategyScopeUseCases>,
    pub strategy_task_step_evidence: StrategyTaskStepEvidenceUseCases,
    pub trades: TradeUseCases,
    pub valuations: ValuationUseCases,
}
