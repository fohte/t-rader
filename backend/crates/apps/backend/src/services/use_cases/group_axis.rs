use std::sync::Arc;

use core_application::group_axis::GroupAxisUseCases;
use gateway_postgres::PostgresGroupAxisRepository;

use super::UseCases;

impl UseCases {
    pub fn group_axes(&self) -> GroupAxisUseCases {
        GroupAxisUseCases::new(
            self.unit_of_work.clone(),
            Arc::new(PostgresGroupAxisRepository::new(self.db.clone())),
        )
    }
}
