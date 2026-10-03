use std::sync::{Arc, Mutex};

use axum_test::TestServer;
use chrono::{DateTime, TimeZone, Utc};
use sea_orm::ActiveModelTrait;
use sea_orm::ActiveValue::{NotSet, Set};
use sea_orm::ConnectionTrait;
use uuid::Uuid;

use crate::{
    build_agent_webhook_state, build_external_webhook_state, build_http_state, create_router,
};
use core_application::agent_task_client::SharedAgentTaskClient;
use core_application::daily_bar_source::{DailyBarSource, DailyBarSourceError, DateRange};
use core_application::kata_exec::SharedKataExecutor;
use core_domain::bar::Bar;
use core_domain::instrument::Instrument;
use entrypoint_frontend_api::FrontendApiState;
use gateway_postgres::DatabaseHandle;
use gateway_postgres::entities::sea_orm_active_enums::{StrategyTaskPhase, StrategyTaskStepStatus};
use gateway_postgres::entities::{
    group_axis, stock, stock_group, stock_group_member, strategy, strategy_task,
    strategy_task_step, trigger,
};

mod annotation;
mod note;
pub use annotation::set_test_annotation_execution_step_id;
pub use note::{
    find_current_note_version, insert_test_note, insert_test_note_as, insert_test_note_in_scope,
    insert_test_note_with_execution_id, insert_test_note_with_status,
    set_test_note_version_execution_id,
};

/// テスト全体で共通の webhook トークン。agent webhook の認証テストで使う。
pub const TEST_AGENT_WEBHOOK_TOKEN: &str = "test-agent-webhook-token";

/// テスト用 router state を組み立てる。
struct TestRouterState {
    use_cases: crate::services::use_cases::UseCases,
    app_state: FrontendApiState,
    agent_webhook_state: entrypoint_agent_webhook::AgentWebhookState,
    external_webhook_state: entrypoint_external_webhook::ExternalWebhookState,
}

fn base_state(db: DatabaseHandle, agent_task_client: SharedAgentTaskClient) -> TestRouterState {
    let use_cases = crate::services::use_cases::build_use_cases(db);
    let app_state = build_http_state(&use_cases, agent_task_client.clone(), None, None);
    TestRouterState {
        agent_webhook_state: build_agent_webhook_state(&use_cases, TEST_AGENT_WEBHOOK_TOKEN),
        external_webhook_state: build_external_webhook_state(&use_cases, agent_task_client),
        use_cases,
        app_state,
    }
}

fn create_test_router(states: TestRouterState, db: DatabaseHandle) -> axum::Router {
    create_router(
        states.app_state,
        states.agent_webhook_state,
        states.external_webhook_state,
        states.use_cases,
        None,
        db,
    )
}

/// `#[backend_test_macros::database_test]` から注入された transaction を使って TestServer を作成する。
pub async fn create_test_server(db: DatabaseHandle) -> TestServer {
    let states = base_state(db.clone(), FrontendApiState::disabled_agent_task_client());
    let router = create_test_router(states, db);
    TestServer::new(router).expect("failed to create test server")
}

pub mod agent_config {
    use core_application::agent_config::{AgentConfig, AgentConfigUseCaseError};

    use super::DatabaseHandle;

    pub async fn create(
        db: &DatabaseHandle,
        purpose: String,
    ) -> Result<AgentConfig, AgentConfigUseCaseError> {
        crate::services::use_cases::build_use_cases(db.clone())
            .agent_configs()
            .create(purpose)
            .await
    }
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

/// テストで strategy_task を 1 件 seed する。`created_at`/`updated_at` を明示指定できる
/// ため、一覧の並び順を検証するテストで使う。
pub async fn insert_test_strategy_task(
    db: &impl ConnectionTrait,
    strategy_id: Uuid,
    prompt: &str,
    purpose: Option<&str>,
    created_at: DateTime<chrono::FixedOffset>,
) -> Uuid {
    insert_test_strategy_task_with_source(db, strategy_id, "frontend", prompt, purpose, created_at)
        .await
}

async fn insert_test_strategy_task_with_source(
    db: &impl ConnectionTrait,
    strategy_id: Uuid,
    source: &str,
    prompt: &str,
    purpose: Option<&str>,
    created_at: DateTime<chrono::FixedOffset>,
) -> Uuid {
    let task_id = Uuid::new_v4();
    strategy_task::ActiveModel {
        task_id: Set(task_id),
        strategy_id: Set(strategy_id),
        a2a_task_id: Set(None),
        source: Set(source.to_string()),
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

/// テストで strategy_task_step を 1 件 seed し、親の strategy_task ID を返す。
pub async fn insert_test_strategy_task_step(
    db: &impl ConnectionTrait,
    strategy_id: Uuid,
    execution_step_id: Uuid,
) -> Uuid {
    let now = Utc::now().fixed_offset();
    let task_id = insert_test_strategy_task_with_source(
        db,
        strategy_id,
        "sample",
        "sample prompt",
        None,
        now,
    )
    .await;
    strategy_task_step::ActiveModel {
        execution_step_id: Set(execution_step_id),
        task_id: Set(task_id),
        phase_key: Set("sample-phase".to_string()),
        label: Set("Sample step".to_string()),
        model: Set("fictional-model".to_string()),
        status: Set(StrategyTaskStepStatus::Completed),
        item: Set(None),
        item_label: Set(None),
        output: Set(None),
        started_at: Set(now),
        finished_at: Set(Some(now)),
        trace_id: Set("sample-trace".to_string()),
        span_id: Set("sample-span".to_string()),
        error: Set(None),
        seq: NotSet,
    }
    .insert(db)
    .await
    .expect("insert test strategy task step");
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
    purpose: Option<&str>,
) -> Uuid {
    let id = Uuid::new_v4();
    trigger::ActiveModel {
        trigger_id: Set(id),
        strategy_id: Set(Some(strategy_id)),
        purpose: Set(purpose.map(str::to_string)),
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
        purpose: Set(None),
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
        product_category: Set(None),
        created_at: NotSet,
        updated_at: NotSet,
    }
    .insert(db)
    .await
    .expect("insert test stock");
}

/// テストで group_axis と stock_group を seed する。
pub async fn insert_test_group(
    db: &impl ConnectionTrait,
    axis_key: &str,
    group_key: &str,
    name: &str,
) -> String {
    insert_test_group_with_sync_source_code(db, axis_key, group_key, name, None, None)
        .await
        .0
}

pub async fn insert_test_group_membership(
    db: &impl ConnectionTrait,
    stock_id: &str,
    axis_key: &str,
    group_key: &str,
    name: &str,
    sync_source: Option<&str>,
) {
    let (_, group_id) =
        insert_test_group_with_sync_source_code(db, axis_key, group_key, name, sync_source, None)
            .await;
    stock_group_member::ActiveModel {
        stock_id: Set(stock_id.into()),
        group_id: Set(group_id),
        created_at: NotSet,
    }
    .insert(db)
    .await
    .expect("insert test group membership");
}

/// テストで同期元と同期元コードを持つグループを seed する。
pub async fn insert_test_group_with_sync_source_code(
    db: &impl ConnectionTrait,
    axis_key: &str,
    group_key: &str,
    name: &str,
    sync_source: Option<&str>,
    sync_source_code: Option<&str>,
) -> (String, Uuid) {
    let axis_id = Uuid::new_v4();
    group_axis::ActiveModel {
        id: Set(axis_id),
        key: Set(axis_key.into()),
        name: Set("Sample Axis".into()),
        description: Set("Sample axis for tests".into()),
        sync_source: Set(sync_source.map(str::to_string)),
    }
    .insert(db)
    .await
    .expect("insert test group axis");
    let group_id = Uuid::new_v4();
    stock_group::ActiveModel {
        id: Set(group_id),
        axis_id: Set(axis_id),
        key: Set(group_key.into()),
        name: Set(name.into()),
        description: Set(None),
        sync_source_code: Set(sync_source_code.map(str::to_string)),
    }
    .insert(db)
    .await
    .expect("insert test stock group");
    (format!("{axis_key}/{group_key}"), group_id)
}

/// `create_test_server` の `(db, server)` ペア版。agent_task_client は disabled。
pub async fn create_test_server_with_db(db: DatabaseHandle) -> (DatabaseHandle, TestServer) {
    let states = base_state(db.clone(), FrontendApiState::disabled_agent_task_client());
    let router = create_test_router(states, db.clone());
    let server = TestServer::new(router).expect("failed to create test server");
    (db, server)
}

/// kata executor を差し替えて TestServer を作成する
pub async fn create_test_server_with_kata(
    db: DatabaseHandle,
    executor: SharedKataExecutor,
) -> TestServer {
    let mut states = base_state(db.clone(), FrontendApiState::disabled_agent_task_client());
    states.app_state.kata_executor = Some(executor);
    let router = create_test_router(states, db);
    TestServer::new(router).expect("failed to create test server")
}

/// llm_gateway_client を差し替えて TestServer を作成する
pub async fn create_test_server_with_llm_gateway(
    db: DatabaseHandle,
    llm_gateway_base_url: &str,
) -> TestServer {
    let mut states = base_state(db.clone(), FrontendApiState::disabled_agent_task_client());
    states.app_state.llm_gateway_client = Some(Arc::new(
        gateway_litellm::LiteLlmClient::new(llm_gateway_base_url, None)
            .expect("build llm gateway client"),
    ));
    let router = create_test_router(states, db);
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
    let states = base_state(db.clone(), agent_client);
    let router = create_test_router(states, db.clone());
    let server = TestServer::new(router).expect("failed to create test server");
    (db, server)
}

/// Graphile Worker の schema を準備して TestServer を作成する。
pub async fn create_test_server_with_graphile_worker(db: DatabaseHandle) -> TestServer {
    let pool = gateway_postgres::test_support::create_test_pool().await;
    crate::migrations::migrate_graphile_worker_schema(pool)
        .await
        .expect("migrate Graphile Worker schema");
    let states = base_state(db.clone(), FrontendApiState::disabled_agent_task_client());
    let router = create_test_router(states, db);
    TestServer::new(router).expect("failed to create test server")
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

    /// `DailyBarSource` が取得範囲を公開する状態にする。
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
