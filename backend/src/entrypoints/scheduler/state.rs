use std::fmt;
use std::sync::Arc;

use core_application::indicator_observation_source::SharedIndicatorObservationSource;
use gateway_jquants::JQuantsClient;
use sea_orm::DatabaseConnection;

use crate::services::use_cases::UseCases;

#[derive(Clone)]
pub(super) struct SchedulerState {
    pub(super) db: DatabaseConnection,
    pub(super) use_cases: UseCases,
    pub(super) fred_source: Option<SharedIndicatorObservationSource>,
    pub(super) jquants_client: Option<Arc<JQuantsClient>>,
}

impl fmt::Debug for SchedulerState {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SchedulerState")
            .finish_non_exhaustive()
    }
}
