use axum_test::TestServer;
use sqlx::PgPool;

use crate::{AppState, create_router};

/// `#[sqlx::test]` から注入された PgPool を使って TestServer を作成する
pub fn create_test_server(pool: PgPool) -> TestServer {
    let state = AppState { db: pool };
    let router = create_router(state);
    TestServer::new(router).expect("failed to create test server")
}
