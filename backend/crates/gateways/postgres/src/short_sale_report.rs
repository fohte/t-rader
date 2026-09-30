use async_trait::async_trait;
use chrono::NaiveDate;
use core_application::short_sale_report::{
    ShortSaleReportQuery, ShortSaleReportRepository, ShortSaleReportRepositoryError,
};
use core_domain::short_sale_report::ShortSaleReport;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect};

use crate::DatabaseHandle;
use crate::entities::short_sale_report;
use crate::persistence::persistence_error;

#[derive(Clone)]
pub struct PostgresShortSaleReportRepository {
    db: DatabaseHandle,
}

impl PostgresShortSaleReportRepository {
    pub fn new(db: impl Into<DatabaseHandle>) -> Self {
        Self { db: db.into() }
    }
}

#[async_trait]
impl ShortSaleReportRepository for PostgresShortSaleReportRepository {
    async fn find_latest_disc_date(
        &self,
    ) -> Result<Option<NaiveDate>, ShortSaleReportRepositoryError> {
        crate::repositories::short_sale_report::find_latest_disc_date(&self.db)
            .await
            .map_err(repository_error)
    }

    async fn upsert(
        &self,
        reports: Vec<ShortSaleReport>,
    ) -> Result<(), ShortSaleReportRepositoryError> {
        crate::repositories::short_sale_report::upsert_short_sale_reports(&self.db, reports)
            .await
            .map_err(repository_error)
    }

    async fn read(
        &self,
        query: ShortSaleReportQuery,
    ) -> Result<Vec<ShortSaleReport>, ShortSaleReportRepositoryError> {
        let mut select = short_sale_report::Entity::find()
            .filter(short_sale_report::Column::Code.between(query.code_from, query.code_to));
        if let Some(from) = query.from {
            select = select.filter(short_sale_report::Column::DiscDate.gte(from));
        }
        if let Some(to) = query.to {
            select = select.filter(short_sale_report::Column::DiscDate.lte(to));
        }

        select
            .order_by_desc(short_sale_report::Column::DiscDate)
            .order_by_asc(short_sale_report::Column::SsName)
            .order_by_asc(short_sale_report::Column::SsAddr)
            .order_by_asc(short_sale_report::Column::DicName)
            .order_by_asc(short_sale_report::Column::DicAddr)
            .order_by_asc(short_sale_report::Column::FundName)
            .limit(query.limit)
            .all(&self.db)
            .await
            .map(|rows| rows.into_iter().map(to_domain).collect())
            .map_err(repository_error)
    }
}

fn repository_error(error: sea_orm::DbErr) -> ShortSaleReportRepositoryError {
    ShortSaleReportRepositoryError::Database(persistence_error(error))
}

fn to_domain(row: short_sale_report::Model) -> ShortSaleReport {
    ShortSaleReport {
        disc_date: row.disc_date,
        calc_date: row.calc_date,
        code: row.code,
        ss_name: row.ss_name,
        ss_addr: row.ss_addr,
        dic_name: row.dic_name,
        dic_addr: row.dic_addr,
        fund_name: row.fund_name,
        short_position_ratio: row.short_position_ratio,
        short_position_shares: row.short_position_shares,
        short_position_units: row.short_position_units,
        prev_report_date: row.prev_report_date,
        prev_report_ratio: row.prev_report_ratio,
        notes: row.notes,
    }
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;
    use core_application::short_sale_report::{ShortSaleReportQuery, ShortSaleReportRepository};
    use core_domain::short_sale_report::ShortSaleReport;
    use rust_decimal::Decimal;

    use super::PostgresShortSaleReportRepository;

    fn date(day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(2025, 2, day).unwrap()
    }

    fn report(disc_date: NaiveDate, code: &str, reporter: &str, ratio: i64) -> ShortSaleReport {
        ShortSaleReport {
            disc_date,
            calc_date: disc_date,
            code: code.into(),
            ss_name: reporter.into(),
            ss_addr: "Synthetic address".into(),
            dic_name: "Synthetic client".into(),
            dic_addr: "Synthetic address".into(),
            fund_name: "Synthetic fund".into(),
            short_position_ratio: Decimal::new(ratio, 2),
            short_position_shares: ratio,
            short_position_units: ratio,
            prev_report_date: None,
            prev_report_ratio: None,
            notes: String::new(),
        }
    }

    #[backend_test_macros::database_test]
    async fn reads_symbol_range_in_descending_date_order(db: crate::DatabaseHandle) {
        let repository = PostgresShortSaleReportRepository::new(db);
        repository
            .upsert(vec![
                report(date(2), "T1234", "Synthetic B", 5),
                report(date(3), "T1234", "Synthetic B", 6),
                report(date(3), "U1234", "Synthetic A", 7),
            ])
            .await
            .expect("seed reports");

        let reports = repository
            .read(ShortSaleReportQuery {
                code_from: "T0000".into(),
                code_to: "T9999".into(),
                from: Some(date(2)),
                to: Some(date(3)),
                limit: 10,
            })
            .await
            .expect("read reports");

        assert_eq!(
            reports,
            vec![
                report(date(3), "T1234", "Synthetic B", 6),
                report(date(2), "T1234", "Synthetic B", 5),
            ],
        );
    }
}
