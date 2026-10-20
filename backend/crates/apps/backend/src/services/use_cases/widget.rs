use std::sync::Arc;

use core_application::widget::WidgetUseCases;
use gateway_postgres::PostgresWidgetQuerySource;

use super::UseCases;

impl UseCases {
    pub fn widgets(&self) -> WidgetUseCases {
        WidgetUseCases::new(
            Arc::new(PostgresWidgetQuerySource::new(self.db.clone())),
            Arc::new(self.stock_groups()),
        )
    }
}
