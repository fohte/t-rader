use core_application::{
    indicator_observation::IndicatorObservationUseCases,
    indicator_observation_source::SharedIndicatorObservationSource,
    ingest_run_log::SharedIngestRunLog, margin::MarginUseCases, margin_source::SharedMarginSource,
    prediction::PredictionUseCases, short_ratio::ShortRatioUseCases,
    short_sale_report::ShortSaleReportUseCases, short_selling_source::SharedShortSellingSource,
};

#[derive(Clone)]
pub struct SchedulerDependencies {
    pub indicator_observations: IndicatorObservationUseCases,
    pub ingest_run_log: SharedIngestRunLog,
    pub fred_source: Option<SharedIndicatorObservationSource>,
    pub predictions: PredictionUseCases,
    pub short_ratios: ShortRatioUseCases,
    pub short_sale_reports: ShortSaleReportUseCases,
    pub margins: MarginUseCases,
    pub short_selling_source: Option<SharedShortSellingSource>,
    pub margin_source: Option<SharedMarginSource>,
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
