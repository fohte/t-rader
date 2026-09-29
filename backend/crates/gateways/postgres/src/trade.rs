use async_trait::async_trait;
use core_application::trade::{
    NewTrade, Trade, TradeListItem, TradeOrder, TradeQuery, TradeRepository, TradeRepositoryError,
    TradeUpdate,
};
use core_application::unit_of_work::UnitOfWorkTransaction;
use sea_orm::ActiveValue::Set;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect};
use uuid::Uuid;

use crate::DatabaseHandle;
use crate::entities::{trade, trade_note};
use crate::persistence::persistence_error;
use crate::transaction::transaction_ref as postgres_transaction_ref;

#[derive(Clone)]
pub struct PostgresTradeRepository {
    db: DatabaseHandle,
}

impl PostgresTradeRepository {
    pub fn new(db: DatabaseHandle) -> Self {
        Self { db }
    }
}

#[async_trait]
impl TradeRepository for PostgresTradeRepository {
    async fn list(&self, query: TradeQuery) -> Result<Vec<TradeListItem>, TradeRepositoryError> {
        let mut select = trade::Entity::find();
        if let Some(strategy_id) = query.strategy_id {
            select = select.filter(trade::Column::StrategyId.eq(strategy_id));
        }
        if let Some(symbol) = query.symbol {
            select = select.filter(trade::Column::Symbol.eq(symbol));
        }
        if let Some(date_from) = query.date_from {
            select = select.filter(trade::Column::Date.gte(date_from));
        }
        select = match query.order {
            TradeOrder::DateAscending => select
                .order_by_asc(trade::Column::Date)
                .order_by_asc(trade::Column::CreatedAt),
            TradeOrder::DateDescending => select
                .order_by_desc(trade::Column::Date)
                .order_by_desc(trade::Column::CreatedAt),
        };
        if let Some(limit) = query.limit {
            select = select.limit(limit);
        }
        let rows = select.all(&self.db).await.map_err(repository_error)?;
        if !query.include_note_count || rows.is_empty() {
            return Ok(rows
                .into_iter()
                .map(|row| TradeListItem {
                    trade: to_domain(row),
                    note_count: 0,
                })
                .collect());
        }

        let ids: Vec<Uuid> = rows.iter().map(|row| row.id).collect();
        let note_counts: std::collections::HashMap<Uuid, i64> = trade_note::Entity::find()
            .select_only()
            .column(trade_note::Column::TradeId)
            .column_as(trade_note::Column::TradeId.count(), "note_count")
            .filter(trade_note::Column::TradeId.is_in(ids))
            .group_by(trade_note::Column::TradeId)
            .into_tuple()
            .all(&self.db)
            .await
            .map_err(repository_error)?
            .into_iter()
            .collect();

        Ok(rows
            .into_iter()
            .map(|row| TradeListItem {
                note_count: note_counts.get(&row.id).copied().unwrap_or_default(),
                trade: to_domain(row),
            })
            .collect())
    }

    async fn find_by_id(&self, id: Uuid) -> Result<Option<Trade>, TradeRepositoryError> {
        trade::Entity::find_by_id(id)
            .one(&self.db)
            .await
            .map(|row| row.map(to_domain))
            .map_err(repository_error)
    }

    async fn find_by_id_in_transaction(
        &self,
        transaction: &UnitOfWorkTransaction,
        id: Uuid,
    ) -> Result<Option<Trade>, TradeRepositoryError> {
        let transaction = transaction_ref(transaction)?;
        trade::Entity::find_by_id(id)
            .one(transaction)
            .await
            .map(|row| row.map(to_domain))
            .map_err(repository_error)
    }

    async fn insert(
        &self,
        transaction: &UnitOfWorkTransaction,
        trade: NewTrade,
    ) -> Result<Trade, TradeRepositoryError> {
        let transaction = transaction_ref(transaction)?;
        let model = trade::ActiveModel {
            id: Set(trade.id),
            strategy_id: Set(trade.strategy_id),
            symbol: Set(trade.symbol),
            side: Set(trade.side),
            qty: Set(trade.qty),
            price: Set(trade.price),
            fee: Set(trade.fee),
            date: Set(trade.date),
            source: Set(trade.source),
            note: Set(trade.note),
            created_at: sea_orm::ActiveValue::NotSet,
            updated_at: sea_orm::ActiveValue::NotSet,
        };
        trade::Entity::insert(model)
            .exec_with_returning(transaction)
            .await
            .map(to_domain)
            .map_err(repository_error)
    }

    async fn update(
        &self,
        transaction: &UnitOfWorkTransaction,
        trade: TradeUpdate,
    ) -> Result<Trade, TradeRepositoryError> {
        let transaction = transaction_ref(transaction)?;
        let trade = trade.trade;
        let model = trade::ActiveModel {
            id: sea_orm::ActiveValue::Unchanged(trade.id),
            strategy_id: Set(trade.strategy_id),
            symbol: Set(trade.symbol),
            side: Set(trade.side),
            qty: Set(trade.qty),
            price: Set(trade.price),
            fee: Set(trade.fee),
            date: Set(trade.date),
            source: Set(trade.source),
            note: Set(trade.note),
            created_at: sea_orm::ActiveValue::Unchanged(trade.created_at),
            updated_at: Set(trade.updated_at),
        };
        model
            .update(transaction)
            .await
            .map(to_domain)
            .map_err(repository_error)
    }

    async fn delete(
        &self,
        transaction: &UnitOfWorkTransaction,
        id: Uuid,
    ) -> Result<bool, TradeRepositoryError> {
        let transaction = transaction_ref(transaction)?;
        trade::Entity::delete_by_id(id)
            .exec(transaction)
            .await
            .map(|result| result.rows_affected > 0)
            .map_err(repository_error)
    }
}

fn transaction_ref(
    transaction: &UnitOfWorkTransaction,
) -> Result<&sea_orm::DatabaseTransaction, TradeRepositoryError> {
    postgres_transaction_ref(transaction).ok_or(TradeRepositoryError::InvalidTransaction)
}

fn repository_error(error: sea_orm::DbErr) -> TradeRepositoryError {
    TradeRepositoryError::Database(persistence_error(error))
}

fn to_domain(model: trade::Model) -> Trade {
    Trade {
        id: model.id,
        strategy_id: model.strategy_id,
        symbol: model.symbol,
        side: model.side,
        qty: model.qty,
        price: model.price,
        fee: model.fee,
        date: model.date,
        source: model.source,
        note: model.note,
        created_at: model.created_at,
        updated_at: model.updated_at,
    }
}
