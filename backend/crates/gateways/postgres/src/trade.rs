use async_trait::async_trait;
use core_application::change_history::{
    ChangeHistoryError, ChangeHistoryPort, ChangeHistoryRecord,
};
use core_application::persistence::PersistenceError;
use core_application::trade::{
    NewTrade, SharedTradeRepository, Trade, TradeListItem, TradeOrder, TradeQuery, TradeRepository,
    TradeRepositoryError, TradeUpdate, TradeUseCases,
};
use core_application::unit_of_work::{
    SharedUnitOfWork, UnitOfWork, UnitOfWorkError, UnitOfWorkTransaction,
};
use sea_orm::ActiveValue::Set;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DbErr, EntityTrait, QueryFilter, QueryOrder, QuerySelect,
    RuntimeErr, SqlErr, TransactionTrait,
};
use uuid::Uuid;

use crate::DatabaseHandle;
use crate::entities::{change_history, strategy, trade, trade_note};

#[derive(Clone)]
pub struct PostgresUnitOfWork {
    db: DatabaseHandle,
}

impl PostgresUnitOfWork {
    pub fn new(db: DatabaseHandle) -> Self {
        Self { db }
    }
}

#[async_trait]
impl UnitOfWork for PostgresUnitOfWork {
    async fn begin(&self) -> Result<UnitOfWorkTransaction, UnitOfWorkError> {
        self.db
            .begin()
            .await
            .map(UnitOfWorkTransaction::new)
            .map_err(|error| UnitOfWorkError::Begin(persistence_error(error)))
    }

    async fn commit(&self, transaction: UnitOfWorkTransaction) -> Result<(), UnitOfWorkError> {
        let transaction = transaction
            .downcast::<sea_orm::DatabaseTransaction>()
            .map_err(|_| UnitOfWorkError::InvalidTransaction)?;
        transaction
            .commit()
            .await
            .map_err(|error| UnitOfWorkError::Commit(persistence_error(error)))
    }
}

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

    async fn strategy_exists(
        &self,
        transaction: &UnitOfWorkTransaction,
        strategy_id: Uuid,
    ) -> Result<bool, TradeRepositoryError> {
        let transaction = transaction_ref(transaction)?;
        strategy::Entity::find_by_id(strategy_id)
            .one(transaction)
            .await
            .map(|row| row.is_some())
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

pub struct PostgresChangeHistory;

#[async_trait]
impl ChangeHistoryPort for PostgresChangeHistory {
    async fn record(
        &self,
        transaction: &UnitOfWorkTransaction,
        record: ChangeHistoryRecord,
    ) -> Result<(), ChangeHistoryError> {
        let transaction = transaction_ref(transaction).map_err(|error| match error {
            TradeRepositoryError::InvalidTransaction => ChangeHistoryError::InvalidTransaction,
            TradeRepositoryError::Database(error) => ChangeHistoryError::Database(error),
        })?;
        let model = change_history::ActiveModel {
            id: Set(Uuid::new_v4()),
            target_kind: Set(record.target_kind.as_str().to_string()),
            target_id: Set(record.target_id),
            actor_kind: Set(record.actor.kind().to_string()),
            actor_label: Set(record.actor.label().to_string()),
            op: Set(record.op.as_str().to_string()),
            diff_json: Set(record.diff),
            summary: Set(record.summary),
            created_at: sea_orm::ActiveValue::NotSet,
        };
        change_history::Entity::insert(model)
            .exec_without_returning(transaction)
            .await
            .map(|_| ())
            .map_err(|error| ChangeHistoryError::Database(persistence_error(error)))
    }
}

pub fn postgres_trade_use_cases(db: DatabaseHandle) -> TradeUseCases {
    let unit_of_work: SharedUnitOfWork = std::sync::Arc::new(PostgresUnitOfWork::new(db.clone()));
    let repository: SharedTradeRepository = std::sync::Arc::new(PostgresTradeRepository::new(db));
    let change_history: std::sync::Arc<dyn ChangeHistoryPort + Send + Sync> =
        std::sync::Arc::new(PostgresChangeHistory);
    TradeUseCases::new(unit_of_work, repository, change_history)
}

fn transaction_ref(
    transaction: &UnitOfWorkTransaction,
) -> Result<&sea_orm::DatabaseTransaction, TradeRepositoryError> {
    transaction
        .downcast_ref::<sea_orm::DatabaseTransaction>()
        .ok_or(TradeRepositoryError::InvalidTransaction)
}

fn repository_error(error: sea_orm::DbErr) -> TradeRepositoryError {
    TradeRepositoryError::Database(persistence_error(error))
}

fn persistence_error(error: DbErr) -> PersistenceError {
    let message = error.to_string();
    if matches!(&error, DbErr::RecordNotUpdated) {
        return PersistenceError::RecordNotUpdated(message);
    }
    if let Some(sql_error) = error.sql_err() {
        match sql_error {
            SqlErr::ForeignKeyConstraintViolation(_) => {
                return PersistenceError::MissingReference(message);
            }
            SqlErr::UniqueConstraintViolation(_) => {
                return PersistenceError::Conflict(message);
            }
            _ => {}
        }
    }
    let code = match &error {
        DbErr::Exec(RuntimeErr::SqlxError(error)) | DbErr::Query(RuntimeErr::SqlxError(error)) => {
            error.as_database_error().and_then(|error| error.code())
        }
        _ => None,
    };
    match code.as_deref() {
        Some("23503") => PersistenceError::MissingReference(message),
        Some("23505") => PersistenceError::Conflict(message),
        Some("23514") => PersistenceError::ConstraintViolation(message),
        _ => PersistenceError::Database(message),
    }
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
