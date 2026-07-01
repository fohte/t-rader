use axum_test::TestServer;
use migration::{Migrator, MigratorTrait};
use sea_orm::ActiveModelTrait;
use sea_orm::ActiveValue::{NotSet, Set};
use sea_orm::{DatabaseConnection, SqlxPostgresConnector};
use sqlx::PgPool;
use uuid::Uuid;

use crate::entities::sea_orm_active_enums::StrategyAgentStatus;
use crate::entities::strategy;
use crate::kata_exec::SharedKataExecutor;
use crate::kubeopencode::SharedKubeopencodeClient;
use crate::{AppState, create_router};

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

/// `#[sqlx::test]` から注入された PgPool を使って TestServer を作成する
///
/// PgPool を SeaORM の DatabaseConnection に変換し、マイグレーションを実行する。
pub async fn create_test_server(pool: PgPool) -> TestServer {
    let db = create_test_db(pool).await;

    let state = AppState {
        db,
        data_provider: None,
        kubeopencode: AppState::disabled_kubeopencode(),
        kata_executor: None,
        macro_cache: None,
    };
    let router = create_router(state);
    TestServer::new(router).expect("failed to create test server")
}

/// テストで戦略レコードを 1 件 seed する。agent_status は `Ready` 固定。
pub async fn insert_test_strategy(db: &DatabaseConnection, name: &str) -> Uuid {
    let id = Uuid::new_v4();
    strategy::ActiveModel {
        id: Set(id),
        name: Set(name.to_string()),
        description: Set(None),
        sort_order: Set(0),
        agents_md: NotSet,
        skills: NotSet,
        agent_status: Set(StrategyAgentStatus::Ready),
        agent_error: NotSet,
        created_at: NotSet,
        updated_at: NotSet,
    }
    .insert(db)
    .await
    .expect("insert test strategy");
    id
}

/// `create_test_server` の `(db, server)` ペア版。kubeopencode は disabled。
pub async fn create_test_server_with_db(pool: PgPool) -> (DatabaseConnection, TestServer) {
    create_test_server_with_db_and_kube(pool, AppState::disabled_kubeopencode()).await
}

/// kubeopencode クライアントを差し替えて TestServer を作成する
pub async fn create_test_server_with_kube(
    pool: PgPool,
    kube: SharedKubeopencodeClient,
) -> TestServer {
    let (_, server) = create_test_server_with_db_and_kube(pool, kube).await;
    server
}

/// kata executor を差し替えて TestServer を作成する
pub async fn create_test_server_with_kata(
    pool: PgPool,
    executor: SharedKataExecutor,
) -> TestServer {
    let db = create_test_db(pool).await;
    let state = AppState {
        db,
        data_provider: None,
        kubeopencode: AppState::disabled_kubeopencode(),
        kata_executor: Some(executor),
        macro_cache: None,
    };
    let router = create_router(state);
    TestServer::new(router).expect("failed to create test server")
}

/// テスト中に SeaORM 経由で直接行を seed したい場合に、`DatabaseConnection` と
/// `TestServer` をペアで返すバリアント。
pub async fn create_test_server_with_db_and_kube(
    pool: PgPool,
    kube: SharedKubeopencodeClient,
) -> (DatabaseConnection, TestServer) {
    let db = create_test_db(pool).await;

    let state = AppState {
        db: db.clone(),
        data_provider: None,
        kubeopencode: kube,
        kata_executor: None,
        macro_cache: None,
    };
    let router = create_router(state);
    let server = TestServer::new(router).expect("failed to create test server");
    (db, server)
}
