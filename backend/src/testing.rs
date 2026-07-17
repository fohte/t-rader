use std::sync::Arc;

use axum_test::TestServer;
use migration::{Migrator, MigratorTrait};
use sea_orm::ActiveModelTrait;
use sea_orm::ActiveValue::{NotSet, Set};
use sea_orm::{DatabaseConnection, SqlxPostgresConnector};
use sqlx::PgPool;
use uuid::Uuid;

use crate::agent_client::SharedAgentTaskClient;
use crate::entities::strategy;
use crate::kata_exec::SharedKataExecutor;
use crate::{AppState, create_router};

/// テスト全体で共通の webhook トークン。`create_test_server_with_state` でこの値を
/// 参照できる。
pub const TEST_AGENT_WEBHOOK_TOKEN: &str = "test-agent-webhook-token";

/// `#[sqlx::test]` から注入された PgPool を SeaORM DatabaseConnection に変換する
///
/// マイグレーションも実行する。HTTP サーバー不要な repository テスト向け。
pub async fn create_test_db(pool: PgPool) -> DatabaseConnection {
    let db = SqlxPostgresConnector::from_sqlx_postgres_pool(pool);

    Migrator::up(&db, None)
        .await
        .expect("failed to run migrations");

    db
}

/// agent_task_client を disabled にした最小構成の `AppState` を組み立てる。
fn base_state(db: DatabaseConnection) -> AppState {
    AppState {
        db,
        data_provider: None,
        agent_task_client: AppState::disabled_agent_task_client(),
        agent_task_notify: Arc::new(tokio::sync::Notify::new()),
        agent_webhook_token: Arc::from(TEST_AGENT_WEBHOOK_TOKEN),
        kata_executor: None,
        macro_cache: None,
    }
}

/// `#[sqlx::test]` から注入された PgPool を使って TestServer を作成する
///
/// PgPool を SeaORM の DatabaseConnection に変換し、マイグレーションを実行する。
pub async fn create_test_server(pool: PgPool) -> TestServer {
    let db = create_test_db(pool).await;
    let router = create_router(base_state(db));
    TestServer::new(router).expect("failed to create test server")
}

/// テストで戦略レコードを 1 件 seed する。
pub async fn insert_test_strategy(db: &DatabaseConnection, name: &str) -> Uuid {
    let id = Uuid::new_v4();
    strategy::ActiveModel {
        id: Set(id),
        name: Set(name.to_string()),
        description: Set(None),
        sort_order: Set(0),
        agents_md: NotSet,
        skills: NotSet,
        created_at: NotSet,
        updated_at: NotSet,
    }
    .insert(db)
    .await
    .expect("insert test strategy");
    id
}

/// `create_test_server` の `(db, server)` ペア版。agent_task_client は disabled。
pub async fn create_test_server_with_db(pool: PgPool) -> (DatabaseConnection, TestServer) {
    let db = create_test_db(pool).await;
    let state = base_state(db.clone());
    let router = create_router(state);
    let server = TestServer::new(router).expect("failed to create test server");
    (db, server)
}

/// kata executor を差し替えて TestServer を作成する
pub async fn create_test_server_with_kata(
    pool: PgPool,
    executor: SharedKataExecutor,
) -> TestServer {
    let db = create_test_db(pool).await;
    let mut state = base_state(db);
    state.kata_executor = Some(executor);
    let router = create_router(state);
    TestServer::new(router).expect("failed to create test server")
}

/// agent_task_client (t-rader-agent 内部 API client) を差し替えて TestServer を作成する
pub async fn create_test_server_with_agent_client(
    pool: PgPool,
    agent_client: SharedAgentTaskClient,
) -> TestServer {
    let (_, server) = create_test_server_with_db_and_agent_client(pool, agent_client).await;
    server
}

/// `create_test_server_with_db` の agent_task_client 差し替え版。
pub async fn create_test_server_with_db_and_agent_client(
    pool: PgPool,
    agent_client: SharedAgentTaskClient,
) -> (DatabaseConnection, TestServer) {
    let db = create_test_db(pool).await;
    let mut state = base_state(db.clone());
    state.agent_task_client = agent_client;
    let router = create_router(state);
    let server = TestServer::new(router).expect("failed to create test server");
    (db, server)
}

/// `AppState` 全体と `TestServer` のペアを返す。webhook token / notify への直接アクセスが
/// 必要なテスト (webhook 受信のような) 向け。
pub async fn create_test_server_with_state(pool: PgPool) -> (AppState, TestServer) {
    let db = create_test_db(pool).await;
    let state = base_state(db);
    let router = create_router(state.clone());
    let server = TestServer::new(router).expect("failed to create test server");
    (state, server)
}
