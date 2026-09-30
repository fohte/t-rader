use std::sync::Arc;

use core_application::change_history::ChangeHistoryUseCases;
use gateway_postgres::PostgresChangeHistoryQuery;

use super::UseCases;

impl UseCases {
    pub fn change_history(&self) -> ChangeHistoryUseCases {
        ChangeHistoryUseCases::new(Arc::new(PostgresChangeHistoryQuery::new(self.db.clone())))
    }
}
