use std::sync::{Arc, Mutex};

use axum_test::TestServer;
use chrono::{DateTime, TimeZone, Utc};
use migration::{Migrator, MigratorTrait};
use sea_orm::ActiveModelTrait;
use sea_orm::ActiveValue::{NotSet, Set};
use sea_orm::{DatabaseConnection, SqlxPostgresConnector};
use sqlx::PgPool;
use uuid::Uuid;

use crate::agent_client::SharedAgentTaskClient;
use crate::data_provider::{DataProvider, DataProviderError, DateRange};
use crate::entities::sea_orm_active_enums::StrategyTaskPhase;
use crate::entities::{strategy, strategy_task, trigger};
use crate::kata_exec::SharedKataExecutor;
use crate::models::{Bar, Instrument};
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
        llm_gateway_client: None,
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

/// テスト用に `POST /api/strategies` で戦略を 1 件作成し、その ID を返す。
pub async fn create_strategy(server: &TestServer, name: &str) -> String {
    let created = server
        .post("/api/strategies")
        .json(&serde_json::json!({ "name": name }))
        .await;
    created.assert_status(axum::http::StatusCode::CREATED);
    created.json::<serde_json::Value>()["id"]
        .as_str()
        .map(str::to_string)
        .expect("id")
}

/// テストで戦略レコードを 1 件 seed する。
pub async fn insert_test_strategy(db: &DatabaseConnection, name: &str) -> Uuid {
    let id = Uuid::new_v4();
    strategy::ActiveModel {
        id: Set(id),
        name: Set(name.to_string()),
        description: Set(None),
        sort_order: Set(0),
        created_at: NotSet,
        updated_at: NotSet,
    }
    .insert(db)
    .await
    .expect("insert test strategy");
    id
}

/// テストで strategy_task を 1 件 seed する。`created_at`/`updated_at` を明示指定できる
/// ため、一覧の並び順を検証するテストで使う。
pub async fn insert_test_strategy_task(
    db: &DatabaseConnection,
    strategy_id: Uuid,
    prompt: &str,
    purpose: Option<&str>,
    created_at: DateTime<chrono::FixedOffset>,
) -> Uuid {
    let task_id = Uuid::new_v4();
    strategy_task::ActiveModel {
        task_id: Set(task_id),
        strategy_id: Set(strategy_id),
        a2a_task_id: Set(None),
        source: Set("frontend".to_string()),
        prompt: Set(prompt.to_string()),
        phase: Set(StrategyTaskPhase::Completed),
        error_summary: Set(None),
        result_text: Set(None),
        deadline_at: Set(created_at + chrono::Duration::minutes(15)),
        steps: Set(serde_json::json!([])),
        purpose: Set(purpose.map(str::to_string)),
        created_at: Set(created_at),
        updated_at: Set(created_at),
    }
    .insert(db)
    .await
    .expect("insert test strategy task");
    task_id
}

/// テストで cron trigger を 1 件 seed する。
pub async fn insert_test_cron_trigger(
    db: &DatabaseConnection,
    strategy_id: Uuid,
    schedule: &str,
    enabled: bool,
    last_fired_at: Option<DateTime<Utc>>,
    prompt_template: &str,
) -> Uuid {
    let id = Uuid::new_v4();
    trigger::ActiveModel {
        trigger_id: Set(id),
        strategy_id: Set(Some(strategy_id)),
        kind: Set("cron".to_string()),
        schedule: Set(Some(schedule.to_string())),
        hook_slug: Set(None),
        event_match: Set(None),
        prompt_template: Set(prompt_template.to_string()),
        enabled: Set(enabled),
        last_fired_at: Set(last_fired_at.map(|dt| dt.fixed_offset())),
        created_at: NotSet,
        updated_at: NotSet,
    }
    .insert(db)
    .await
    .expect("insert test cron trigger");
    id
}

/// テストで hook trigger を 1 件 seed する。
pub async fn insert_test_hook_trigger(
    db: &DatabaseConnection,
    strategy_id: Uuid,
    slug: &str,
    prompt_template: &str,
    event_match: Option<serde_json::Value>,
    enabled: bool,
) -> Uuid {
    let id = Uuid::new_v4();
    trigger::ActiveModel {
        trigger_id: Set(id),
        strategy_id: Set(Some(strategy_id)),
        kind: Set("hook".to_string()),
        schedule: Set(None),
        hook_slug: Set(Some(slug.to_string())),
        event_match: Set(event_match),
        prompt_template: Set(prompt_template.to_string()),
        enabled: Set(enabled),
        last_fired_at: NotSet,
        created_at: NotSet,
        updated_at: NotSet,
    }
    .insert(db)
    .await
    .expect("insert test hook trigger");
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

/// llm_gateway_client を差し替えて TestServer を作成する
pub async fn create_test_server_with_llm_gateway(
    pool: PgPool,
    llm_gateway_base_url: &str,
) -> TestServer {
    let db = create_test_db(pool).await;
    let mut state = base_state(db);
    state.llm_gateway_client = Some(
        crate::services::litellm_client::LiteLlmClient::new(llm_gateway_base_url, None)
            .expect("build llm gateway client"),
    );
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

/// テスト用のモックデータプロバイダー
///
/// `calls` に `fetch_daily_bars` へ渡された instrument_id を記録するため、
/// どの銘柄が実際にバックフィルされたかをテストで検証できる。
pub struct MockProvider {
    bars: Vec<Bar>,
    instruments: Vec<Instrument>,
    pub calls: Mutex<Vec<String>>,
}

impl MockProvider {
    pub fn new() -> Self {
        Self {
            bars: Vec::new(),
            instruments: Vec::new(),
            calls: Mutex::new(Vec::new()),
        }
    }

    pub fn with_bars(mut self, bars: Vec<Bar>) -> Self {
        self.bars = bars;
        self
    }

    pub fn with_instruments(mut self, instruments: Vec<Instrument>) -> Self {
        self.instruments = instruments;
        self
    }
}

impl Default for MockProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl DataProvider for MockProvider {
    async fn fetch_daily_bars(
        &self,
        instrument_id: &str,
        range: &DateRange,
    ) -> Result<Vec<Bar>, DataProviderError> {
        self.calls
            .lock()
            .expect("lock")
            .push(instrument_id.to_string());

        let exists = self.instruments.iter().any(|i| i.id == instrument_id);
        if !exists {
            return Err(DataProviderError::NotFound(format!(
                "instrument '{instrument_id}' not found"
            )));
        }

        let from_dt = Utc.from_utc_datetime(&range.from.and_hms_opt(0, 0, 0).unwrap_or_default());
        let to_exclusive = range.to.succ_opt().unwrap_or(range.to);
        let to_dt = Utc.from_utc_datetime(&to_exclusive.and_hms_opt(0, 0, 0).unwrap_or_default());

        let mut bars: Vec<Bar> = self
            .bars
            .iter()
            .filter(|b| {
                b.instrument_id == instrument_id && b.timestamp >= from_dt && b.timestamp < to_dt
            })
            .cloned()
            .collect();

        bars.sort_by_key(|b| b.timestamp);
        Ok(bars)
    }

    async fn fetch_instrument(&self, instrument_id: &str) -> Result<Instrument, DataProviderError> {
        self.instruments
            .iter()
            .find(|i| i.id == instrument_id)
            .cloned()
            .ok_or_else(|| {
                DataProviderError::NotFound(format!("instrument '{instrument_id}' not found"))
            })
    }
}
