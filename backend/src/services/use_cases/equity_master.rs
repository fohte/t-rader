use std::sync::Arc;

use core_application::equity_master::EquityMasterUseCases;
use gateway_postgres::PostgresEquityMasterRepository;

use super::UseCases;

impl UseCases {
    pub fn equity_master(&self) -> EquityMasterUseCases {
        EquityMasterUseCases::new(
            self.unit_of_work.clone(),
            Arc::new(PostgresEquityMasterRepository),
        )
    }
}
