use axum_test::TestServer;
use migration::{Migrator, MigratorTrait};
use sea_orm::SqlxPostgresConnector;
use sqlx::PgPool;

use crate::{AppState, create_router};

/// `#[sqlx::test]` から注入された PgPool を使って TestServer を作成する
///
/// PgPool を SeaORM の DatabaseConnection に変換し、マイグレーションを実行する。
pub async fn create_test_server(pool: PgPool) -> TestServer {
    let db = SqlxPostgresConnector::from_sqlx_postgres_pool(pool);

    Migrator::up(&db, None)
        .await
        .expect("failed to run migrations");

    let state = AppState { db };
    let router = create_router(state);
    TestServer::new(router).expect("failed to create test server")
}
