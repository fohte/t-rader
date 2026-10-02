use async_trait::async_trait;
use core_application::trade::{
    NewTrade, NewTradeNoteLink, Trade, TradeListItem, TradeMatchQuery, TradeNoteLink,
    TradeNoteReference, TradeOrder, TradeQuery, TradeRepository, TradeRepositoryError, TradeUpdate,
};
use core_application::unit_of_work::UnitOfWorkTransaction;
use sea_orm::ActiveValue::{NotSet, Set};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder, QuerySelect,
};
use uuid::Uuid;

use crate::DatabaseHandle;
use crate::entities::{stock, trade, trade_note};
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
        if (!query.include_note_count && !query.include_note_references) || rows.is_empty() {
            return Ok(rows
                .into_iter()
                .map(|row| TradeListItem {
                    trade: to_domain(row),
                    note_count: 0,
                    note_references: Vec::new(),
                })
                .collect());
        }

        let ids: Vec<Uuid> = rows.iter().map(|row| row.id).collect();
        let mut note_counts = std::collections::HashMap::new();
        let mut note_references: std::collections::HashMap<Uuid, Vec<TradeNoteReference>> =
            std::collections::HashMap::new();
        if query.include_note_references {
            let links: Vec<(Uuid, Uuid, Uuid)> = trade_note::Entity::find()
                .select_only()
                .column(trade_note::Column::TradeId)
                .column(trade_note::Column::NoteId)
                .column(trade_note::Column::NoteVersionId)
                .filter(trade_note::Column::TradeId.is_in(ids))
                .order_by_asc(trade_note::Column::CreatedAt)
                .order_by_asc(trade_note::Column::NoteId)
                .into_tuple()
                .all(&self.db)
                .await
                .map_err(repository_error)?;
            for (trade_id, note_id, note_version_id) in links {
                *note_counts.entry(trade_id).or_default() += 1;
                note_references
                    .entry(trade_id)
                    .or_default()
                    .push(TradeNoteReference {
                        note_id,
                        note_version_id,
                    });
            }
        } else {
            note_counts = trade_note::Entity::find()
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
        }

        Ok(rows
            .into_iter()
            .map(|row| TradeListItem {
                note_count: note_counts.get(&row.id).copied().unwrap_or_default(),
                note_references: note_references.remove(&row.id).unwrap_or_default(),
                trade: to_domain(row),
            })
            .collect())
    }

    async fn count_matching_trades(
        &self,
        query: &TradeMatchQuery,
    ) -> Result<usize, TradeRepositoryError> {
        count_matching_trades(&self.db, query)
            .await
            .map_err(repository_error)
    }

    async fn count_matching_trades_in_transaction(
        &self,
        transaction: &UnitOfWorkTransaction,
        query: &TradeMatchQuery,
    ) -> Result<usize, TradeRepositoryError> {
        count_matching_trades(transaction_ref(transaction)?, query)
            .await
            .map_err(repository_error)
    }

    async fn ensure_stock(
        &self,
        transaction: &UnitOfWorkTransaction,
        symbol: &str,
        name: &str,
    ) -> Result<(), TradeRepositoryError> {
        let transaction = transaction_ref(transaction)?;
        // SBI CSV の銘柄名は独自表記のため、既存のマスタ名は更新しない。
        if stock::Entity::find_by_id(symbol.to_string())
            .one(transaction)
            .await
            .map_err(repository_error)?
            .is_some()
        {
            return Ok(());
        }
        // 空欄の CSV 名を銘柄名として保存しないよう、銘柄コードを代わりに使う。
        let trimmed_name = name.trim();
        let resolved_name = if trimmed_name.is_empty() {
            symbol
        } else {
            trimmed_name
        };
        let model = stock::ActiveModel {
            id: Set(symbol.to_string()),
            name: Set(resolved_name.to_string()),
            market: Set(None),
            product_category: Set(None),
            created_at: NotSet,
            updated_at: NotSet,
        };
        // 並行 import が同じ銘柄を追加した場合は、一意制約違反を成功扱いにする。
        if let Err(error) = stock::Entity::insert(model).exec(transaction).await {
            if matches!(
                error.sql_err(),
                Some(sea_orm::SqlErr::UniqueConstraintViolation(_))
            ) {
                return Ok(());
            }
            return Err(repository_error(error));
        }
        Ok(())
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

    async fn list_note_links(
        &self,
        transaction: &UnitOfWorkTransaction,
        trade_id: Uuid,
    ) -> Result<Vec<TradeNoteLink>, TradeRepositoryError> {
        let transaction = transaction_ref(transaction)?;
        trade_note::Entity::find()
            .filter(trade_note::Column::TradeId.eq(trade_id))
            .order_by_asc(trade_note::Column::CreatedAt)
            .all(transaction)
            .await
            .map(|rows| rows.into_iter().map(to_trade_note_link).collect())
            .map_err(repository_error)
    }

    async fn insert_note_link(
        &self,
        transaction: &UnitOfWorkTransaction,
        link: NewTradeNoteLink,
    ) -> Result<TradeNoteLink, TradeRepositoryError> {
        let transaction = transaction_ref(transaction)?;
        trade_note::Entity::insert(trade_note::ActiveModel {
            trade_id: Set(link.trade_id),
            note_id: Set(link.note_id),
            note_version_id: Set(link.note_version_id),
            created_at: NotSet,
        })
        .exec_with_returning(transaction)
        .await
        .map(to_trade_note_link)
        .map_err(repository_error)
    }

    async fn delete_note_link(
        &self,
        transaction: &UnitOfWorkTransaction,
        trade_id: Uuid,
        note_id: Uuid,
    ) -> Result<bool, TradeRepositoryError> {
        let transaction = transaction_ref(transaction)?;
        trade_note::Entity::delete_many()
            .filter(trade_note::Column::TradeId.eq(trade_id))
            .filter(trade_note::Column::NoteId.eq(note_id))
            .exec(transaction)
            .await
            .map(|result| result.rows_affected > 0)
            .map_err(repository_error)
    }
}

async fn count_matching_trades<C: ConnectionTrait>(
    connection: &C,
    query: &TradeMatchQuery,
) -> Result<usize, sea_orm::DbErr> {
    let count = trade::Entity::find()
        .filter(trade::Column::Date.eq(query.date))
        .filter(trade::Column::Symbol.eq(&query.symbol))
        .filter(trade::Column::Side.eq(&query.side))
        .filter(trade::Column::Qty.eq(query.qty))
        .filter(trade::Column::Price.eq(query.price))
        .count(connection)
        .await?;
    Ok(count as usize)
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

fn to_trade_note_link(model: trade_note::Model) -> TradeNoteLink {
    TradeNoteLink {
        trade_id: model.trade_id,
        note_id: model.note_id,
        note_version_id: model.note_version_id,
        created_at: model.created_at,
    }
}
