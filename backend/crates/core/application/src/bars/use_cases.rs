use core_domain::bar::Bar;

use crate::unit_of_work::SharedUnitOfWork;

use super::error::BarsUseCaseError;
use super::repository::SharedBarsRepository;
use super::types::{BarsByInstrumentsQuery, BarsQuery};

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
