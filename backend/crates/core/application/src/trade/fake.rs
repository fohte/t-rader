use std::collections::HashMap;

use async_trait::async_trait;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::unit_of_work::{FakeTransaction, UnitOfWorkTransaction};

use super::repository::{TradeRepository, TradeRepositoryError};
use super::types::{NewTrade, Trade, TradeUpdate};
use super::types::{TradeListItem, TradeOrder, TradeQuery};

#[derive(Default)]
pub struct FakeTradeRepository {
    pub trades: Mutex<HashMap<Uuid, Trade>>,
    pub transaction_ids: Mutex<Vec<Uuid>>,
}

impl FakeTradeRepository {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn insert_trade(&self, trade: Trade) {
        self.trades.lock().await.insert(trade.id, trade);
    }
}

#[async_trait]
impl TradeRepository for FakeTradeRepository {
    async fn list(&self, query: TradeQuery) -> Result<Vec<TradeListItem>, TradeRepositoryError> {
        let trades = self.trades.lock().await;
        let mut rows: Vec<TradeListItem> = trades
            .values()
            .filter(|trade| {
                query.strategy_id.is_none_or(|id| trade.strategy_id == id)
                    && query
                        .symbol
                        .as_deref()
                        .is_none_or(|symbol| trade.symbol == symbol)
                    && query.date_from.is_none_or(|date| trade.date >= date)
            })
            .cloned()
            .map(|trade| TradeListItem {
                trade,
                note_count: 0,
            })
            .collect();
        rows.sort_by(|left, right| match query.order {
            TradeOrder::DateAscending => left
                .trade
                .date
                .cmp(&right.trade.date)
                .then_with(|| left.trade.created_at.cmp(&right.trade.created_at)),
            TradeOrder::DateDescending => right
                .trade
                .date
                .cmp(&left.trade.date)
                .then_with(|| right.trade.created_at.cmp(&left.trade.created_at)),
        });
        if let Some(limit) = query.limit {
            rows.truncate(usize::try_from(limit).unwrap_or(usize::MAX));
        }
        Ok(rows)
    }

    async fn find_by_id(&self, id: Uuid) -> Result<Option<Trade>, TradeRepositoryError> {
        Ok(self.trades.lock().await.get(&id).cloned())
    }

    async fn find_by_id_in_transaction(
        &self,
        transaction: &UnitOfWorkTransaction,
        id: Uuid,
    ) -> Result<Option<Trade>, TradeRepositoryError> {
        self.record_transaction(transaction).await?;
        self.find_by_id(id).await
    }

    async fn insert(
        &self,
        transaction: &UnitOfWorkTransaction,
        trade: NewTrade,
    ) -> Result<Trade, TradeRepositoryError> {
        self.record_transaction(transaction).await?;
        let now = chrono::Utc::now().fixed_offset();
        let trade = Trade {
            id: trade.id,
            strategy_id: trade.strategy_id,
            symbol: trade.symbol,
            side: trade.side,
            qty: trade.qty,
            price: trade.price,
            fee: trade.fee,
            date: trade.date,
            source: trade.source,
            note: trade.note,
            created_at: now,
            updated_at: now,
        };
        self.trades.lock().await.insert(trade.id, trade.clone());
        Ok(trade)
    }

    async fn update(
        &self,
        transaction: &UnitOfWorkTransaction,
        trade: TradeUpdate,
    ) -> Result<Trade, TradeRepositoryError> {
        self.record_transaction(transaction).await?;
        self.trades
            .lock()
            .await
            .insert(trade.trade.id, trade.trade.clone());
        Ok(trade.trade)
    }

    async fn delete(
        &self,
        transaction: &UnitOfWorkTransaction,
        id: Uuid,
    ) -> Result<bool, TradeRepositoryError> {
        self.record_transaction(transaction).await?;
        Ok(self.trades.lock().await.remove(&id).is_some())
    }
}

impl FakeTradeRepository {
    async fn record_transaction(
        &self,
        transaction: &UnitOfWorkTransaction,
    ) -> Result<(), TradeRepositoryError> {
        let transaction_id = transaction
            .downcast_ref::<FakeTransaction>()
            .map(|transaction| transaction.id)
            .ok_or(TradeRepositoryError::InvalidTransaction)?;
        self.transaction_ids.lock().await.push(transaction_id);
        Ok(())
    }
}
