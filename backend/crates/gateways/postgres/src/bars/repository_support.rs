use core_application::bars::{BarsQuery, BarsRepositoryError};
use core_application::unit_of_work::UnitOfWorkTransaction;

use crate::persistence::persistence_error;
use crate::repositories::bars::BarsQuery as PostgresBarsQuery;
use crate::transaction::transaction_ref as postgres_transaction_ref;

pub(super) fn to_postgres_query(query: BarsQuery) -> PostgresBarsQuery {
    PostgresBarsQuery {
        instrument_id: query.instrument_id,
        timeframe: query.timeframe,
        from: query.from,
        to: query.to,
    }
}

pub(super) fn transaction_ref(
    transaction: &UnitOfWorkTransaction,
) -> Result<&sea_orm::DatabaseTransaction, BarsRepositoryError> {
    postgres_transaction_ref(transaction).ok_or(BarsRepositoryError::InvalidTransaction)
}

pub(super) fn repository_error(error: sea_orm::DbErr) -> BarsRepositoryError {
    BarsRepositoryError::Database(persistence_error(error))
}
