use std::sync::Arc;

use core_application::short_sale_report::ShortSaleReportUseCases;
use gateway_postgres::PostgresShortSaleReportRepository;

use super::UseCases;

impl UseCases {
    pub fn short_sale_reports(&self) -> ShortSaleReportUseCases {
        ShortSaleReportUseCases::new(Arc::new(PostgresShortSaleReportRepository::new(
            self.db.clone(),
        )))
    }
}
