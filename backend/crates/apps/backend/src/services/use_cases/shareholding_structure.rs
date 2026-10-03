use std::sync::Arc;

use core_application::shareholding_structure::ShareholdingStructureUseCases;
use gateway_postgres::PostgresShareholdingStructureRepository;

use super::UseCases;

impl UseCases {
    pub fn shareholding_structures(&self) -> ShareholdingStructureUseCases {
        ShareholdingStructureUseCases::new(
            self.unit_of_work.clone(),
            Arc::new(PostgresShareholdingStructureRepository::new(
                self.db.clone(),
            )),
        )
    }
}
