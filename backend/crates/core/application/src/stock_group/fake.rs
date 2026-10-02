use std::collections::{HashMap, HashSet};

use async_trait::async_trait;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::persistence::PersistenceError;
use crate::unit_of_work::{FakeTransaction, UnitOfWorkTransaction};

use super::repository::{GroupAxis, StockGroupRepository, StockGroupRepositoryError};
use super::types::{NewStockGroup, StockGroup, StockGroupMembership};

#[derive(Default)]
pub struct FakeStockGroupRepository {
    axes: Mutex<HashMap<String, GroupAxis>>,
    groups: Mutex<HashMap<Uuid, StockGroup>>,
    stocks: Mutex<HashSet<String>>,
    members: Mutex<HashSet<(Uuid, String)>>,
    insert_conflict: Mutex<Option<String>>,
    pub transaction_ids: Mutex<Vec<Uuid>>,
}

impl FakeStockGroupRepository {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn insert_axis(&self, key: &str, sync_source: Option<&str>) -> GroupAxis {
        let axis = GroupAxis {
            id: Uuid::new_v4(),
            key: key.to_owned(),
            sync_source: sync_source.map(str::to_owned),
        };
        self.axes
            .lock()
            .await
            .insert(axis.key.clone(), axis.clone());
        axis
    }

    pub async fn insert_stock(&self, stock_id: &str) {
        self.stocks.lock().await.insert(stock_id.to_owned());
    }

    pub async fn conflict_next_insert(&self, message: &str) {
        *self.insert_conflict.lock().await = Some(message.to_owned());
    }

    async fn record_transaction(
        &self,
        transaction: &UnitOfWorkTransaction,
    ) -> Result<(), StockGroupRepositoryError> {
        let transaction_id = transaction
            .downcast_ref::<FakeTransaction>()
            .map(|transaction| transaction.id)
            .ok_or(StockGroupRepositoryError::InvalidTransaction)?;
        self.transaction_ids.lock().await.push(transaction_id);
        Ok(())
    }
}

#[async_trait]
impl StockGroupRepository for FakeStockGroupRepository {
    async fn find_axis_by_key(
        &self,
        transaction: &UnitOfWorkTransaction,
        axis_key: &str,
    ) -> Result<Option<GroupAxis>, StockGroupRepositoryError> {
        self.record_transaction(transaction).await?;
        Ok(self.axes.lock().await.get(axis_key).cloned())
    }

    async fn find_group(
        &self,
        transaction: &UnitOfWorkTransaction,
        axis_id: Uuid,
        axis_key: &str,
        group_key: &str,
    ) -> Result<Option<StockGroup>, StockGroupRepositoryError> {
        self.record_transaction(transaction).await?;
        Ok(self
            .groups
            .lock()
            .await
            .values()
            .find(|group| group.axis_id == axis_id && group.key == group_key)
            .cloned()
            .map(|mut group| {
                group.axis_key = axis_key.to_owned();
                group
            }))
    }

    async fn insert_group(
        &self,
        transaction: &UnitOfWorkTransaction,
        group: NewStockGroup,
    ) -> Result<StockGroup, StockGroupRepositoryError> {
        self.record_transaction(transaction).await?;
        if let Some(message) = self.insert_conflict.lock().await.take() {
            return Err(PersistenceError::Conflict(message).into());
        }
        let group = StockGroup {
            id: group.id,
            axis_id: group.axis_id,
            axis_key: group.axis_key,
            key: group.key,
            name: group.name,
            description: group.description,
        };
        self.groups.lock().await.insert(group.id, group.clone());
        Ok(group)
    }

    async fn update_group(
        &self,
        transaction: &UnitOfWorkTransaction,
        group: StockGroup,
    ) -> Result<StockGroup, StockGroupRepositoryError> {
        self.record_transaction(transaction).await?;
        self.groups.lock().await.insert(group.id, group.clone());
        Ok(group)
    }

    async fn stock_exists(
        &self,
        transaction: &UnitOfWorkTransaction,
        stock_id: &str,
    ) -> Result<bool, StockGroupRepositoryError> {
        self.record_transaction(transaction).await?;
        Ok(self.stocks.lock().await.contains(stock_id))
    }

    async fn list_stock_ids(
        &self,
        transaction: &UnitOfWorkTransaction,
        group_id: Uuid,
    ) -> Result<Vec<String>, StockGroupRepositoryError> {
        self.record_transaction(transaction).await?;
        let mut stock_ids: Vec<_> = self
            .members
            .lock()
            .await
            .iter()
            .filter(|(member_group_id, _)| *member_group_id == group_id)
            .map(|(_, stock_id)| stock_id.clone())
            .collect();
        stock_ids.sort();
        Ok(stock_ids)
    }

    async fn list_memberships(
        &self,
        transaction: &UnitOfWorkTransaction,
        stock_ids: &[String],
        axis_keys: &[String],
    ) -> Result<Vec<StockGroupMembership>, StockGroupRepositoryError> {
        self.record_transaction(transaction).await?;
        let stock_ids = stock_ids.iter().collect::<HashSet<_>>();
        let axis_keys = axis_keys.iter().collect::<HashSet<_>>();
        let groups = self.groups.lock().await;
        let axes = self.axes.lock().await;
        let members = self.members.lock().await;
        Ok(members
            .iter()
            .filter(|(_, stock_id)| stock_ids.contains(stock_id))
            .filter_map(|(group_id, stock_id)| {
                let group = groups.get(group_id)?;
                let axis = axes.get(&group.axis_key)?;
                axis_keys.contains(&axis.key).then(|| StockGroupMembership {
                    axis_key: axis.key.clone(),
                    group_key: group.key.clone(),
                    stock_id: stock_id.clone(),
                })
            })
            .collect())
    }

    async fn add_stock(
        &self,
        transaction: &UnitOfWorkTransaction,
        group_id: Uuid,
        stock_id: &str,
    ) -> Result<bool, StockGroupRepositoryError> {
        self.record_transaction(transaction).await?;
        Ok(self
            .members
            .lock()
            .await
            .insert((group_id, stock_id.to_owned())))
    }

    async fn remove_stock(
        &self,
        transaction: &UnitOfWorkTransaction,
        group_id: Uuid,
        stock_id: &str,
    ) -> Result<bool, StockGroupRepositoryError> {
        self.record_transaction(transaction).await?;
        Ok(self
            .members
            .lock()
            .await
            .remove(&(group_id, stock_id.to_owned())))
    }
}
