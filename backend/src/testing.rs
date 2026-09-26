use std::{
    str::FromStr,
    sync::{Arc, Mutex},
};

use axum_test::TestServer;
use chrono::{DateTime, TimeZone, Utc};
use sea_orm::ActiveModelTrait;
use sea_orm::ActiveValue::{NotSet, Set};
use sea_orm::{
    ConnectionTrait, EntityTrait, SqlxPostgresConnector, TransactionSession, TransactionTrait,
};
use sqlx::{
    AssertSqlSafe, Connection as _, PgConnection, postgres::PgConnectOptions,
    postgres::PgPoolOptions,
};
use uuid::Uuid;

use crate::agent_client::SharedAgentTaskClient;
use crate::data_provider::{DailyBarSource, DailyBarSourceError, DateRange, SharedDailyBarSource};
use crate::database::DatabaseHandle;
use crate::entities::sea_orm_active_enums::StrategyTaskPhase;
use crate::entities::{note, note_version, stock, strategy, strategy_task, trigger};
use crate::kata_exec::SharedKataExecutor;
use crate::models::{Bar, Instrument};
use crate::{AppState, create_router};
use migration::{Migrator, MigratorTrait};

/// テスト全体で共通の webhook トークン。`create_test_server_with_state` でこの値を
/// 参照できる。
pub const TEST_AGENT_WEBHOOK_TOKEN: &str = "test-agent-webhook-token";

static TEST_DATABASE_INITIALIZED: tokio::sync::OnceCell<()> = tokio::sync::OnceCell::const_new();

/// テストごとに独立した rollback transaction を作る。
pub async fn create_test_transaction(test_name: &'static str) -> DatabaseHandle {
    TEST_DATABASE_INITIALIZED
        .get_or_init(initialize_test_database)
        .await;

    let application_name = test_application_name(test_name);
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect_with(test_database_options().application_name(&application_name))
        .await
        .expect("connect to shared test database");
    let db = SqlxPostgresConnector::from_sqlx_postgres_pool(pool);
    DatabaseHandle::from(db.begin().await.expect("begin test transaction"))
}

async fn initialize_test_database() {
    let test_database = test_database_name();
    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let base_options = PgConnectOptions::from_str(&database_url).expect("parse DATABASE_URL");
    let mut admin = PgConnection::connect_with(&base_options.clone().database("postgres"))
        .await
        .expect("connect to PostgreSQL admin database");

    sqlx::query_scalar::<_, bool>(
        "SELECT pg_advisory_lock(hashtext('t-rader-test-database'), hashtext('migration')) IS NULL",
    )
    .fetch_one(&mut admin)
    .await
    .expect("lock shared test database migration");

    let database_exists = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS (SELECT 1 FROM pg_database WHERE datname = $1)",
    )
    .bind(&test_database)
    .fetch_one(&mut admin)
    .await
    .expect("check shared test database");
    if !database_exists {
        sqlx::query(AssertSqlSafe(format!(
            "CREATE DATABASE {}",
            quote_identifier(&test_database)
        )))
        .execute(&mut admin)
        .await
        .expect("create shared test database");
    }

    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect_with(base_options.database(&test_database))
        .await
        .expect("connect to shared test database for migrations");
    let db = SqlxPostgresConnector::from_sqlx_postgres_pool(pool);
    Migrator::up(&db, None)
        .await
        .expect("run shared test database migrations");
    drop(db);

    sqlx::query_scalar::<_, bool>(
        "SELECT pg_advisory_unlock(hashtext('t-rader-test-database'), hashtext('migration'))",
    )
    .fetch_one(&mut admin)
    .await
    .expect("unlock shared test database migration");
}

fn test_database_options() -> PgConnectOptions {
    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    PgConnectOptions::from_str(&database_url)
        .expect("parse DATABASE_URL")
        .database(&test_database_name())
}

const APPLICATION_NAME_PREFIX: &str = "dbtest:";
const APPLICATION_NAME_MAX_BYTES: usize = 63;

fn test_application_name(test_name: &str) -> String {
    // PostgreSQL は application_name を 63 byte で切るため、末尾にあるテスト名を残す。
    let max_suffix_bytes = APPLICATION_NAME_MAX_BYTES - APPLICATION_NAME_PREFIX.len();
    let suffix_start = test_name
        .char_indices()
        .find_map(|(index, _)| (test_name.len() - index <= max_suffix_bytes).then_some(index))
        .unwrap_or(0);
    format!("{APPLICATION_NAME_PREFIX}{}", &test_name[suffix_start..])
}

// 別 worktree のテストが古い migration source の DB を使うことがあるため、異なる hash の DB を共存させる。
fn test_database_name() -> String {
    format!("t_rader_test_{}", env!("MIGRATION_SOURCE_HASH"))
}

fn quote_identifier(identifier: &str) -> String {
    format!("\"{}\"", identifier.replace('"', "\"\""))
}

/// agent_task_client を disabled にした最小構成の `AppState` を組み立てる。
fn base_state(db: DatabaseHandle) -> AppState {
    AppState {
        db,
        daily_bar_source: None,
        jquants_client: None,
        agent_task_client: AppState::disabled_agent_task_client(),
        agent_task_notify: Arc::new(tokio::sync::Notify::new()),
        agent_webhook_token: Arc::from(TEST_AGENT_WEBHOOK_TOKEN),
        kata_executor: None,
        llm_gateway_client: None,
    }
}

/// `#[backend_test_macros::database_test]` から注入された transaction を使って TestServer を作成する。
pub async fn create_test_server(db: DatabaseHandle) -> TestServer {
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
pub async fn insert_test_strategy(db: &impl ConnectionTrait, name: &str) -> Uuid {
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

/// テストで note を 1 件 seed する。
pub async fn insert_test_note(
    db: &(impl ConnectionTrait + sea_orm::TransactionTrait),
    strategy_id: Uuid,
    title: &str,
    body_md: &str,
) -> Uuid {
    insert_test_note_in_scope(db, Some(strategy_id), title, body_md).await
}

pub async fn insert_test_note_in_scope(
    db: &(impl ConnectionTrait + sea_orm::TransactionTrait),
    strategy_id: Option<Uuid>,
    title: &str,
    body_md: &str,
) -> Uuid {
    insert_test_note_with_options(db, strategy_id, title, body_md, None, "human", "unread").await
}

pub async fn insert_test_note_with_status(
    db: &(impl ConnectionTrait + sea_orm::TransactionTrait),
    strategy_id: Uuid,
    title: &str,
    body_md: &str,
    status: &str,
) -> Uuid {
    insert_test_note_with_options(db, Some(strategy_id), title, body_md, None, "human", status)
        .await
}

pub async fn insert_test_note_with_execution_id(
    db: &(impl ConnectionTrait + sea_orm::TransactionTrait),
    strategy_id: Uuid,
    title: &str,
    body_md: &str,
    execution_id: &str,
) -> Uuid {
    insert_test_note_with_options(
        db,
        Some(strategy_id),
        title,
        body_md,
        Some(execution_id.to_string()),
        "llm",
        "unread",
    )
    .await
}

async fn insert_test_note_with_options(
    db: &(impl ConnectionTrait + sea_orm::TransactionTrait),
    strategy_id: Option<Uuid>,
    title: &str,
    body_md: &str,
    execution_id: Option<String>,
    created_by_kind: &str,
    status: &str,
) -> Uuid {
    let id = Uuid::new_v4();
    let txn = db.begin().await.expect("begin test note transaction");
    note::Entity::insert(note::ActiveModel {
        id: Set(id),
        strategy_id: Set(strategy_id),
        kind: Set(None),
        trigger: Set(None),
        trigger_label: Set(None),
        created_at: NotSet,
        updated_at: NotSet,
        execution_id: Set(execution_id.clone()),
    })
    .exec_without_returning(&txn)
    .await
    .expect("insert test note");
    let version = crate::services::note_versions::append_version(
        &txn,
        id,
        crate::services::note_versions::AppendVersion {
            title: title.to_string(),
            body_md: body_md.to_string(),
            frontmatter_json: serde_json::json!({}),
            graphs_json: serde_json::json!([]),
            created_by_kind: created_by_kind.to_string(),
            execution_id,
            change_reason: None,
            change_diff: None,
            actor: crate::services::change_history::Actor::Human,
        },
    )
    .await
    .expect("append test note version");
    if status != version.status {
        note_version::ActiveModel {
            id: Set(version.id),
            status: Set(status.to_string()),
            ..Default::default()
        }
        .update(&txn)
        .await
        .expect("set test note version status");
    }
    txn.commit().await.expect("commit test note transaction");
    id
}

/// テストで strategy_task を 1 件 seed する。`created_at`/`updated_at` を明示指定できる
/// ため、一覧の並び順を検証するテストで使う。
pub async fn insert_test_strategy_task(
    db: &impl ConnectionTrait,
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
        purpose: Set(purpose.map(str::to_string)),
        as_of: Set(Some(created_at)),
        auto_resumed_at: NotSet,
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
    db: &impl ConnectionTrait,
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
    db: &impl ConnectionTrait,
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

/// テストで stock を 1 件 seed する。
pub async fn insert_test_stock(db: &impl ConnectionTrait, id: &str, name: &str) {
    stock::ActiveModel {
        id: Set(id.to_string()),
        name: Set(name.to_string()),
        market: Set(None),
        sector_id: Set(None),
        product_category: Set(None),
        created_at: NotSet,
        updated_at: NotSet,
    }
    .insert(db)
    .await
    .expect("insert test stock");
}

/// `create_test_server` の `(db, server)` ペア版。agent_task_client は disabled。
pub async fn create_test_server_with_db(db: DatabaseHandle) -> (DatabaseHandle, TestServer) {
    let state = base_state(db.clone());
    let router = create_router(state);
    let server = TestServer::new(router).expect("failed to create test server");
    (db, server)
}

/// data_provider を差し替えて TestServer を作成する
pub async fn create_test_server_with_jquants_client(
    db: DatabaseHandle,
    client: Arc<crate::data_provider::jquants::JQuantsClient>,
) -> TestServer {
    let mut state = base_state(db);
    let source: SharedDailyBarSource = client.clone();
    state.daily_bar_source = Some(source);
    state.jquants_client = Some(client);
    let router = create_router(state);
    TestServer::new(router).expect("failed to create test server")
}

/// kata executor を差し替えて TestServer を作成する
pub async fn create_test_server_with_kata(
    db: DatabaseHandle,
    executor: SharedKataExecutor,
) -> TestServer {
    let mut state = base_state(db);
    state.kata_executor = Some(executor);
    let router = create_router(state);
    TestServer::new(router).expect("failed to create test server")
}

/// llm_gateway_client を差し替えて TestServer を作成する
pub async fn create_test_server_with_llm_gateway(
    db: DatabaseHandle,
    llm_gateway_base_url: &str,
) -> TestServer {
    let mut state = base_state(db);
    state.llm_gateway_client = Some(Arc::new(
        crate::services::litellm_client::LiteLlmClient::new(llm_gateway_base_url, None)
            .expect("build llm gateway client"),
    ));
    let router = create_router(state);
    TestServer::new(router).expect("failed to create test server")
}

/// agent_task_client (t-rader-agent 内部 API client) を差し替えて TestServer を作成する
pub async fn create_test_server_with_agent_client(
    db: DatabaseHandle,
    agent_client: SharedAgentTaskClient,
) -> TestServer {
    let (_, server) = create_test_server_with_db_and_agent_client(db, agent_client).await;
    server
}

/// `create_test_server_with_db` の agent_task_client 差し替え版。
pub async fn create_test_server_with_db_and_agent_client(
    db: DatabaseHandle,
    agent_client: SharedAgentTaskClient,
) -> (DatabaseHandle, TestServer) {
    let mut state = base_state(db.clone());
    state.agent_task_client = agent_client;
    let router = create_router(state);
    let server = TestServer::new(router).expect("failed to create test server");
    (db, server)
}

/// `AppState` 全体と `TestServer` のペアを返す。webhook token / notify への直接アクセスが
/// 必要なテスト (webhook 受信のような) 向け。
pub async fn create_test_server_with_state(db: DatabaseHandle) -> (AppState, TestServer) {
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
    known_fetchable_range: Option<(chrono::NaiveDate, chrono::NaiveDate)>,
    pub calls: Mutex<Vec<String>>,
}

impl MockProvider {
    pub fn new() -> Self {
        Self {
            bars: Vec::new(),
            instruments: Vec::new(),
            known_fetchable_range: None,
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

    /// 契約範囲を検出済みの状態にする (未設定時は `None`)
    pub fn with_known_fetchable_range(
        mut self,
        from: chrono::NaiveDate,
        to: chrono::NaiveDate,
    ) -> Self {
        self.known_fetchable_range = Some((from, to));
        self
    }
}

impl Default for MockProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl DailyBarSource for MockProvider {
    async fn fetch_daily_bars(
        &self,
        instrument_id: &str,
        range: &DateRange,
    ) -> Result<Vec<Bar>, DailyBarSourceError> {
        self.calls
            .lock()
            .expect("lock")
            .push(instrument_id.to_string());

        let exists = self.instruments.iter().any(|i| i.id == instrument_id);
        if !exists {
            return Err(DailyBarSourceError::NotFound(format!(
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

    fn known_fetchable_range(&self) -> Option<(chrono::NaiveDate, chrono::NaiveDate)> {
        self.known_fetchable_range
    }
}
