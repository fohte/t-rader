use std::sync::Arc;

use core_domain::bar::Bar;
use uuid::Uuid;

use crate::ingest_run_log::SharedIngestRunLog;
use crate::unit_of_work::SharedUnitOfWork;

use super::clock::{BarsIngestClock, SystemBarsIngestClock};
use super::error::BarsUseCaseError;
use super::repository::SharedBarsRepository;
use super::types::{BarsByInstrumentsQuery, BarsQuery};
use super::write_lock::{SharedSipRefetchLease, SharedUsStockBarsWriteLock};

#[derive(Clone)]
pub struct BarsUseCases {
    pub(super) unit_of_work: SharedUnitOfWork,
    pub(super) repository: SharedBarsRepository,
}

impl BarsUseCases {
    pub fn new(unit_of_work: SharedUnitOfWork, repository: SharedBarsRepository) -> Self {
        Self {
            unit_of_work,
            repository,
        }
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

#[derive(Clone)]
pub struct UsStockBarsIngestUseCases {
    pub(super) bars: BarsUseCases,
    pub(super) us_stock_bars_write_lock: SharedUsStockBarsWriteLock,
    pub(super) us_stock_sip_refetch_lease: SharedSipRefetchLease,
    pub(super) clock: Arc<dyn BarsIngestClock>,
    pub(super) ingest_run: Option<(Uuid, SharedIngestRunLog)>,
}

impl UsStockBarsIngestUseCases {
    pub fn new(
        bars: BarsUseCases,
        us_stock_bars_write_lock: SharedUsStockBarsWriteLock,
        us_stock_sip_refetch_lease: SharedSipRefetchLease,
    ) -> Self {
        Self::with_clock(
            bars,
            us_stock_bars_write_lock,
            us_stock_sip_refetch_lease,
            Arc::new(SystemBarsIngestClock::new()),
        )
    }

    pub(super) fn with_clock(
        bars: BarsUseCases,
        us_stock_bars_write_lock: SharedUsStockBarsWriteLock,
        us_stock_sip_refetch_lease: SharedSipRefetchLease,
        clock: Arc<dyn BarsIngestClock>,
    ) -> Self {
        Self {
            bars,
            us_stock_bars_write_lock,
            us_stock_sip_refetch_lease,
            clock,
            ingest_run: None,
        }
    }

    pub fn for_ingest_run(&self, run_id: Uuid, ingest_run_log: SharedIngestRunLog) -> Self {
        let mut use_cases = self.clone();
        use_cases.ingest_run = Some((run_id, ingest_run_log));
        use_cases
    }
}
