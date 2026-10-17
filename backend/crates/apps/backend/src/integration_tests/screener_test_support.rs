use gateway_postgres::DatabaseHandle;
use sea_orm::ConnectionTrait;

pub(crate) async fn refresh_screener_snapshot(db: &DatabaseHandle) {
    // database_test は rollback transaction 内で動くため、fixture の refresh では CONCURRENTLY を使えない。
    db.execute_unprepared("REFRESH MATERIALIZED VIEW screener_stock_snapshot")
        .await
        .expect("refresh screener snapshot");
}
