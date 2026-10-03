use std::sync::Arc;

use core_application::earnings_schedule::EarningsScheduleUseCases;
use gateway_postgres::PostgresEarningsScheduleRepository;

use super::UseCases;

impl UseCases {
    pub fn earnings_schedules(&self) -> EarningsScheduleUseCases {
        EarningsScheduleUseCases::new(Arc::new(PostgresEarningsScheduleRepository::new(
            self.db.clone(),
        )))
    }
}
