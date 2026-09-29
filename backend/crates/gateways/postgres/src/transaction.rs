use core_application::unit_of_work::UnitOfWorkTransaction;
use sea_orm::DatabaseTransaction;

pub(crate) fn transaction_ref(transaction: &UnitOfWorkTransaction) -> Option<&DatabaseTransaction> {
    transaction.downcast_ref::<DatabaseTransaction>()
}
