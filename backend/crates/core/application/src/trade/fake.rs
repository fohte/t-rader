use std::collections::HashMap;

use async_trait::async_trait;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::unit_of_work::{FakeTransaction, UnitOfWorkTransaction};

use super::repository::{TradeRepository, TradeRepositoryError};
use super::types::{
    NewTrade, NewTradeNoteLink, Trade, TradeMatchQuery, TradeNoteLink, TradeUpdate,
};
use super::types::{TradeListItem, TradeOrder, TradeQuery};

#[derive(Default)]
pub struct FakeTradeRepository {
    pub trades: Mutex<HashMap<Uuid, Trade>>,
    pub note_links: Mutex<Vec<TradeNoteLink>>,
    pub stocks: Mutex<HashMap<String, String>>,
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
                note_references: Vec::new(),
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

    async fn count_matching_trades(
        &self,
        query: &TradeMatchQuery,
    ) -> Result<usize, TradeRepositoryError> {
        Ok(self
            .trades
            .lock()
            .await
            .values()
            .filter(|trade| {
                trade.date == query.date
                    && trade.symbol == query.symbol
                    && trade.side == query.side
                    && trade.qty == query.qty
                    && trade.price == query.price
            })
            .count())
    }

    async fn count_matching_trades_in_transaction(
        &self,
        transaction: &UnitOfWorkTransaction,
        query: &TradeMatchQuery,
    ) -> Result<usize, TradeRepositoryError> {
        self.record_transaction(transaction).await?;
        self.count_matching_trades(query).await
    }

    async fn ensure_stock(
        &self,
        transaction: &UnitOfWorkTransaction,
        symbol: &str,
        name: &str,
    ) -> Result<(), TradeRepositoryError> {
        self.record_transaction(transaction).await?;
        let resolved_name = if name.trim().is_empty() {
            symbol
        } else {
            name.trim()
        };
        self.stocks
            .lock()
            .await
            .entry(symbol.to_string())
            .or_insert_with(|| resolved_name.to_string());
        Ok(())
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

    async fn list_note_links(
        &self,
        transaction: &UnitOfWorkTransaction,
        trade_id: Uuid,
    ) -> Result<Vec<TradeNoteLink>, TradeRepositoryError> {
        self.record_transaction(transaction).await?;
        let mut links: Vec<_> = self
            .note_links
            .lock()
            .await
            .iter()
            .filter(|link| link.trade_id == trade_id)
            .cloned()
            .collect();
        links.sort_by_key(|link| link.created_at);
        Ok(links)
    }

    async fn insert_note_link(
        &self,
        transaction: &UnitOfWorkTransaction,
        link: NewTradeNoteLink,
    ) -> Result<TradeNoteLink, TradeRepositoryError> {
        self.record_transaction(transaction).await?;
        let created = TradeNoteLink {
            trade_id: link.trade_id,
            note_id: link.note_id,
            note_version_id: link.note_version_id,
            created_at: chrono::Utc::now().fixed_offset(),
        };
        self.note_links.lock().await.push(created.clone());
        Ok(created)
    }

    async fn delete_note_link(
        &self,
        transaction: &UnitOfWorkTransaction,
        trade_id: Uuid,
        note_id: Uuid,
    ) -> Result<bool, TradeRepositoryError> {
        self.record_transaction(transaction).await?;
        let mut links = self.note_links.lock().await;
        let Some(index) = links
            .iter()
            .position(|link| link.trade_id == trade_id && link.note_id == note_id)
        else {
            return Ok(false);
        };
        links.remove(index);
        Ok(true)
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
