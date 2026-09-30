use core_application::agent_task_client::SharedAgentTaskClient;
use core_application::{
    indicator_observation::IndicatorObservationUseCases,
    indicator_observation_source::SharedIndicatorObservationSource, margin::MarginUseCases,
    margin_source::SharedMarginSource, prediction::PredictionUseCases,
    short_ratio::ShortRatioUseCases, short_sale_report::ShortSaleReportUseCases,
    short_selling_source::SharedShortSellingSource, strategy_task::StrategyTaskUseCases,
    trigger::TriggerUseCases,
};

#[derive(Clone)]
pub struct SchedulerDependencies {
    pub indicator_observations: IndicatorObservationUseCases,
    pub fred_source: Option<SharedIndicatorObservationSource>,
    pub predictions: PredictionUseCases,
    pub short_ratios: ShortRatioUseCases,
    pub short_sale_reports: ShortSaleReportUseCases,
    pub margins: MarginUseCases,
    pub short_selling_source: Option<SharedShortSellingSource>,
    pub margin_source: Option<SharedMarginSource>,
    pub strategy_tasks: StrategyTaskUseCases,
    pub triggers: TriggerUseCases,
    pub agent_task_client: SharedAgentTaskClient,
}

#[derive(Clone)]
pub(crate) struct SchedulerState {
    pub(super) dependencies: SchedulerDependencies,
}

impl std::fmt::Debug for SchedulerState {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SchedulerState")
            .finish_non_exhaustive()
    }
}
