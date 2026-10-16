use std::sync::Arc;

use core_domain::bar::Bar;
use uuid::Uuid;

use crate::ingest_run_log::SharedIngestRunLog;
use crate::unit_of_work::SharedUnitOfWork;

use super::clock::{BarsIngestClock, SystemBarsIngestClock};
use super::error::BarsUseCaseError;
use super::repository::SharedBarsRepository;
use super::types::{BarsByInstrumentsQuery, BarsQuery};

#[derive(Clone)]
pub struct BarsUseCases {
    pub(super) unit_of_work: SharedUnitOfWork,
    pub(super) repository: SharedBarsRepository,
    pub(super) clock: Arc<dyn BarsIngestClock>,
    pub(super) ingest_run: Option<(Uuid, SharedIngestRunLog)>,
}

impl BarsUseCases {
    pub fn new(unit_of_work: SharedUnitOfWork, repository: SharedBarsRepository) -> Self {
        Self::with_clock(
            unit_of_work,
            repository,
            Arc::new(SystemBarsIngestClock::new()),
        )
    }

    pub(super) fn with_clock(
        unit_of_work: SharedUnitOfWork,
        repository: SharedBarsRepository,
        clock: Arc<dyn BarsIngestClock>,
    ) -> Self {
        Self {
            unit_of_work,
            repository,
            clock,
            ingest_run: None,
        }
    }

    pub fn for_ingest_run(&self, run_id: Uuid, ingest_run_log: SharedIngestRunLog) -> Self {
        let mut use_cases = self.clone();
        use_cases.ingest_run = Some((run_id, ingest_run_log));
        use_cases
    }

    pub async fn find_bars(&self, query: BarsQuery) -> Result<Vec<Bar>, BarsUseCaseError> {
        self.repository.find_bars(query).await.map_err(Into::into)
    }

    pub async fn find_bars_by_instruments(
        &self,
        query: BarsByInstrumentsQuery,
    ) -> Result<Vec<Bar>, BarsUseCaseError> {
        self.repository
            .find_bars_by_instruments(query)
            .await
            .map_err(Into::into)
    }
}
