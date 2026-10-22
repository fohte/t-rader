use sea_orm::{ConnectionTrait, DatabaseTransaction, QueryResult, Statement, TransactionTrait};

use crate::DatabaseHandle;

use super::{WidgetQueryError, query_error};

pub(super) async fn query_all_with_timeout(
    db: &DatabaseHandle,
    statement: Statement,
) -> Result<Vec<QueryResult>, WidgetQueryError> {
    let transaction: DatabaseTransaction = db.begin().await.map_err(query_error)?;
    transaction
        .execute_unprepared("SET LOCAL statement_timeout = '5s'")
        .await
        .map_err(query_error)?;
    let rows = transaction
        .query_all_raw(statement)
        .await
        .map_err(query_error)?;
    transaction.commit().await.map_err(query_error)?;
    Ok(rows)
}
