use std::{
    collections::{HashMap, HashSet},
    sync::Mutex,
};

use async_trait::async_trait;
use chrono::NaiveDate;
use core_domain::margin::{MarginAlertRecord, MarginInterestRecord};

use super::repository::{MarginRepository, MarginRepositoryError};
use super::types::{MarginQuery, MarginReadResult};
use crate::persistence::PersistenceError;

#[derive(Default)]
pub struct FakeMarginRepository {
    pub interest: Mutex<Vec<MarginInterestRecord>>,
    pub alerts: Mutex<Vec<MarginAlertRecord>>,
    pub fail_interest_upserts_on: Mutex<HashSet<NaiveDate>>,
    pub fail_alert_upserts_on: Mutex<HashSet<NaiveDate>>,
}

impl FakeMarginRepository {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl MarginRepository for FakeMarginRepository {
    async fn upsert_margin_interest(
        &self,
        records: Vec<MarginInterestRecord>,
    ) -> Result<(), MarginRepositoryError> {
        let failing_dates = self
            .fail_interest_upserts_on
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if records
            .iter()
            .any(|record| failing_dates.contains(&record.date))
        {
            return Err(fake_database_error());
        }
        drop(failing_dates);
        let mut stored = self
            .interest
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        for record in records {
            if let Some(existing) = stored.iter_mut().find(|existing| {
                existing.date == record.date
                    && existing.code == record.code
                    && existing.iss_type == record.iss_type
            }) {
                *existing = record;
            } else {
                stored.push(record);
            }
        }
        Ok(())
    }

    async fn find_latest_margin_interest_date(
        &self,
    ) -> Result<Option<NaiveDate>, MarginRepositoryError> {
        Ok(self
            .interest
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .iter()
            .map(|record| record.date)
            .max())
    }

    async fn upsert_margin_alert(
        &self,
        records: Vec<MarginAlertRecord>,
    ) -> Result<(), MarginRepositoryError> {
        let failing_dates = self
            .fail_alert_upserts_on
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if records
            .iter()
            .any(|record| failing_dates.contains(&record.pub_date))
        {
            return Err(fake_database_error());
        }
        drop(failing_dates);
        let mut stored = self
            .alerts
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        for record in records {
            if let Some(existing) = stored.iter_mut().find(|existing| {
                existing.pub_date == record.pub_date && existing.code == record.code
            }) {
                *existing = record;
            } else {
                stored.push(record);
            }
        }
        Ok(())
    }

    async fn find_latest_margin_alert_pub_date(
        &self,
    ) -> Result<Option<NaiveDate>, MarginRepositoryError> {
        Ok(self
            .alerts
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .iter()
            .map(|record| record.pub_date)
            .max())
    }

    async fn read(&self, query: MarginQuery) -> Result<MarginReadResult, MarginRepositoryError> {
        let interest = self
            .interest
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut matching_interest: Vec<_> = interest
            .iter()
            .filter(|record| matches_symbol_prefix(&record.code, &query.symbol))
            .filter(|record| query.from.is_none_or(|from| record.date >= from))
            .filter(|record| query.to.is_none_or(|to| record.date <= to))
            .cloned()
            .collect();
        matching_interest.sort_by(|left, right| {
            right
                .date
                .cmp(&left.date)
                .then_with(|| left.code.cmp(&right.code))
                .then_with(|| left.iss_type.cmp(&right.iss_type))
        });
        matching_interest.truncate(query.limit as usize);

        let alerts = self
            .alerts
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut latest_by_application_date = HashMap::new();
        for record in alerts.iter().filter(|record| {
            matches_symbol_prefix(&record.code, &query.symbol)
                && query.from.is_none_or(|from| record.app_date >= from)
                && query.to.is_none_or(|to| record.app_date <= to)
        }) {
            let key = (record.app_date, record.code.as_str());
            let replace = latest_by_application_date
                .get(&key)
                .is_none_or(|existing: &&MarginAlertRecord| record.pub_date > existing.pub_date);
            if replace {
                latest_by_application_date.insert(key, record);
            }
        }
        let mut matching_alerts: Vec<_> =
            latest_by_application_date.into_values().cloned().collect();
        matching_alerts.sort_by(|left, right| {
            right
                .app_date
                .cmp(&left.app_date)
                .then_with(|| left.code.cmp(&right.code))
        });
        matching_alerts.truncate(query.limit as usize);

        Ok(MarginReadResult {
            interest: matching_interest,
            alerts: matching_alerts,
        })
    }
}

fn fake_database_error() -> MarginRepositoryError {
    PersistenceError::Database("fake persistence failure".into()).into()
}

fn matches_symbol_prefix(code: &str, symbol: &str) -> bool {
    code.chars().take(4).eq(symbol.chars()) && symbol.chars().count() == 4
}
