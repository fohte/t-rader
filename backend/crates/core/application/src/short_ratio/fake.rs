use std::sync::{Mutex, MutexGuard};

use crate::persistence::PersistenceError;
use async_trait::async_trait;
use chrono::NaiveDate;
use core_domain::short_ratio::ShortRatio;

use super::repository::{ShortRatioRepository, ShortRatioRepositoryError};
use super::types::ShortRatioQuery;

#[derive(Default)]
pub struct FakeShortRatioRepository {
    pub rows: Mutex<Vec<ShortRatio>>,
    pub queries: Mutex<Vec<ShortRatioQuery>>,
    upsert_error: Mutex<Option<String>>,
}

impl FakeShortRatioRepository {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn fail_upserts_with(&self, message: impl Into<String>) {
        *lock(&self.upsert_error) = Some(message.into());
    }
}

#[async_trait]
impl ShortRatioRepository for FakeShortRatioRepository {
    async fn find_latest_date(&self) -> Result<Option<NaiveDate>, ShortRatioRepositoryError> {
        Ok(lock(&self.rows).iter().map(|row| row.date).max())
    }

    async fn upsert(&self, ratios: Vec<ShortRatio>) -> Result<(), ShortRatioRepositoryError> {
        if let Some(message) = lock(&self.upsert_error).take() {
            return Err(ShortRatioRepositoryError::Database(
                PersistenceError::Database(message),
            ));
        }
        let mut rows = lock(&self.rows);
        for ratio in ratios {
            if let Some(existing) = rows.iter_mut().find(|existing| {
                existing.date == ratio.date && existing.sector33_code == ratio.sector33_code
            }) {
                *existing = ratio;
            } else {
                rows.push(ratio);
            }
        }
        Ok(())
    }

    async fn read(
        &self,
        query: ShortRatioQuery,
    ) -> Result<Vec<ShortRatio>, ShortRatioRepositoryError> {
        lock(&self.queries).push(query.clone());
        let mut rows: Vec<_> = lock(&self.rows)
            .iter()
            .filter(|row| row.sector33_code == query.sector33_code)
            .filter(|row| query.from.is_none_or(|from| row.date >= from))
            .filter(|row| query.to.is_none_or(|to| row.date <= to))
            .cloned()
            .collect();
        rows.sort_by_key(|row| std::cmp::Reverse(row.date));
        rows.truncate(query.limit as usize);
        Ok(rows)
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}
