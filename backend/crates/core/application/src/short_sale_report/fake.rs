use std::sync::{Mutex, MutexGuard};

use async_trait::async_trait;
use chrono::NaiveDate;
use core_domain::short_sale_report::ShortSaleReport;

use crate::persistence::PersistenceError;

use super::repository::{ShortSaleReportRepository, ShortSaleReportRepositoryError};
use super::types::ShortSaleReportQuery;

#[derive(Default)]
pub struct FakeShortSaleReportRepository {
    pub rows: Mutex<Vec<ShortSaleReport>>,
    pub queries: Mutex<Vec<ShortSaleReportQuery>>,
    upsert_error: Mutex<Option<String>>,
}

impl FakeShortSaleReportRepository {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn fail_upserts_with(&self, message: impl Into<String>) {
        *lock(&self.upsert_error) = Some(message.into());
    }
}

#[async_trait]
impl ShortSaleReportRepository for FakeShortSaleReportRepository {
    async fn find_latest_disc_date(
        &self,
    ) -> Result<Option<NaiveDate>, ShortSaleReportRepositoryError> {
        Ok(lock(&self.rows).iter().map(|row| row.disc_date).max())
    }

    async fn upsert(
        &self,
        reports: Vec<ShortSaleReport>,
    ) -> Result<(), ShortSaleReportRepositoryError> {
        if let Some(message) = lock(&self.upsert_error).take() {
            return Err(ShortSaleReportRepositoryError::Database(
                PersistenceError::Database(message),
            ));
        }
        let mut rows = lock(&self.rows);
        for report in reports {
            if let Some(existing) = rows.iter_mut().find(|existing| same_key(existing, &report)) {
                *existing = report;
            } else {
                rows.push(report);
            }
        }
        Ok(())
    }

    async fn read(
        &self,
        query: ShortSaleReportQuery,
    ) -> Result<Vec<ShortSaleReport>, ShortSaleReportRepositoryError> {
        lock(&self.queries).push(query.clone());
        let mut rows: Vec<_> = lock(&self.rows)
            .iter()
            .filter(|row| row.code >= query.code_from && row.code <= query.code_to)
            .filter(|row| query.from.is_none_or(|from| row.disc_date >= from))
            .filter(|row| query.to.is_none_or(|to| row.disc_date <= to))
            .cloned()
            .collect();
        rows.sort_by(|left, right| {
            right
                .disc_date
                .cmp(&left.disc_date)
                .then_with(|| left.ss_name.cmp(&right.ss_name))
                .then_with(|| left.ss_addr.cmp(&right.ss_addr))
                .then_with(|| left.dic_name.cmp(&right.dic_name))
                .then_with(|| left.dic_addr.cmp(&right.dic_addr))
                .then_with(|| left.fund_name.cmp(&right.fund_name))
        });
        rows.truncate(query.limit as usize);
        Ok(rows)
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn same_key(left: &ShortSaleReport, right: &ShortSaleReport) -> bool {
    left.disc_date == right.disc_date
        && left.calc_date == right.calc_date
        && left.code == right.code
        && left.ss_name == right.ss_name
        && left.ss_addr == right.ss_addr
        && left.dic_name == right.dic_name
        && left.dic_addr == right.dic_addr
        && left.fund_name == right.fund_name
}
