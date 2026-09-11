//! 戦略タスクの phase 監視 (polling)
//!
//! 一定 interval で `strategy_task.phase IN ('pending', 'running')` の行を LIST し、
//! t-rader-agent の内部 API (`GET /internal/tasks/:task_id`) を照会して結果を DB に反映する。
//! t-rader-agent からの webhook 受信は `notify` 経由で polling を即時発火させるための最適化に
//! 過ぎず、決着の正 (最終的な整合性を保証する経路) は本 polling である。
//!
//! `deadline_at` を過ぎても決着しない行 (内部 API 到達不能、投入自体の記録漏れを含む) は
//! failed に確定し、沈黙したまま残ることを防ぐ。

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, FixedOffset, Utc};
use sea_orm::ActiveValue::{NotSet, Set};
use sea_orm::sea_query::OnConflict;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder};
use tokio::sync::{Notify, Semaphore};
use uuid::Uuid;

use crate::agent_client::{AgentTaskError, AgentTaskState, AgentTaskStatus, SharedAgentTaskClient};
use crate::entities::sea_orm_active_enums::{StrategyTaskPhase, StrategyTaskStepStatus};
use crate::entities::{strategy_task, strategy_task_step};

pub const DEFAULT_INTERVAL: Duration = Duration::from_secs(10);

/// 1 tick あたりに同時実行する内部 API 問い合わせ数の上限。1 件の遅延・タイムアウトが
/// 他の reconcile をブロックしないよう、行ごとに `get` を並列化する。t-rader-agent に
/// 同時接続を投げすぎないよう上限を設ける。
const MAX_CONCURRENT_STATUS_FETCHES: usize = 8;

/// A2A TaskState を strategy_task の phase に写像する。
/// 戦略タスクは 1 shot 実行 (再開なし) のため、input-required も failed 扱いとする。
fn phase_for_state(state: AgentTaskState) -> StrategyTaskPhase {
    match state {
        AgentTaskState::Submitted | AgentTaskState::Working => StrategyTaskPhase::Running,
        AgentTaskState::Completed => StrategyTaskPhase::Completed,
        AgentTaskState::InputRequired
        | AgentTaskState::Canceled
        | AgentTaskState::Failed
        | AgentTaskState::Rejected => StrategyTaskPhase::Failed,
    }
}

fn error_summary_for(status: &AgentTaskStatus, phase: &StrategyTaskPhase) -> Option<String> {
    if *phase != StrategyTaskPhase::Failed {
        return None;
    }
    Some(
        status
            .error_kind
            .clone()
            .unwrap_or_else(|| "agent task failed".to_string()),
    )
}

/// 1 回分の polling を実行する。失敗した個別 task はログに残し、他の task の処理を継続する。
///
/// 戻り値は phase 更新が走った task 数。
pub async fn run_once(db: &DatabaseConnection, agent_client: &SharedAgentTaskClient) -> usize {
    let rows = match strategy_task::Entity::find()
        .filter(
            strategy_task::Column::Phase
                .is_in([StrategyTaskPhase::Pending, StrategyTaskPhase::Running]),
        )
        .order_by_asc(strategy_task::Column::CreatedAt)
        .all(db)
        .await
    {
        Ok(rows) => rows,
        Err(err) => {
            tracing::warn!(error = %err, "failed to list in-flight strategy_task rows");
            return 0;
        }
    };

    let now = Utc::now().fixed_offset();
    let semaphore = Arc::new(Semaphore::new(MAX_CONCURRENT_STATUS_FETCHES));
    let mut handles = Vec::with_capacity(rows.len());
    for row in rows {
        let agent_client = agent_client.clone();
        let db = db.clone();
        let sem = semaphore.clone();
        handles.push(tokio::spawn(async move {
            // semaphore で t-rader-agent への同時接続数を制限する。
            let _permit = match sem.acquire_owned().await {
                Ok(permit) => permit,
                Err(_) => return false,
            };
            reconcile_one(&db, &agent_client, row, now).await
        }));
    }

    let mut updated = 0usize;
    for handle in handles {
        match handle.await {
            Ok(true) => updated += 1,
            Ok(false) => {}
            Err(err) => {
                tracing::warn!(error = %err, "strategy_task reconcile task panicked");
            }
        }
    }
    updated
}

/// 単一行の status 取得 → phase 反映を行う。更新が走った場合のみ `true` を返す。
async fn reconcile_one(
    db: &DatabaseConnection,
    agent_client: &SharedAgentTaskClient,
    row: strategy_task::Model,
    now: DateTime<FixedOffset>,
) -> bool {
    let Some(a2a_task_id) = row.a2a_task_id.clone() else {
        // submit_task の Pending 行 INSERT 後、内部 API 投入前にプロセスが落ちた等で
        // a2a_task_id が記録されないまま孤児化したケース。get のしようがないので
        // deadline のみで確定する。
        if now > row.deadline_at {
            return apply_failed(
                db,
                row,
                "agent task submission was not recorded".to_string(),
            )
            .await;
        }
        return false;
    };

    match agent_client.get(&a2a_task_id).await {
        Ok(status) => apply_status(db, row, status).await,
        Err(err) => {
            // 一時的な到達不能 (NotFound を含む)。t-rader-agent 側の task 作成と backend
            // 側の a2a_task_id 記録は別段階のため、insert 直後の一過性の不整合を誤って
            // 確定させないよう deadline 超過まではリトライに委ねる。超過後は server ごと
            // 長期停止しているとみなして失敗確定する (client 側の最終防衛)。
            if now > row.deadline_at {
                let message = match &err {
                    AgentTaskError::NotFound(_) => format!("agent task {a2a_task_id} not found"),
                    _ => format!("agent task unreachable: {err}"),
                };
                apply_failed(db, row, message).await
            } else {
                tracing::warn!(
                    error = %err,
                    task_id = %row.task_id,
                    a2a_task_id,
                    "failed to fetch agent task status; will retry on next tick",
                );
                false
            }
        }
    }
}

async fn apply_status(
    db: &DatabaseConnection,
    row: strategy_task::Model,
    status: AgentTaskStatus,
) -> bool {
    let new_phase = phase_for_state(status.state);
    let new_error = error_summary_for(&status, &new_phase);
    let new_result_text = status.result_text.or_else(|| row.result_text.clone());
    let new_steps = status.steps.clone();
    apply_phase_logged(db, row, new_phase, new_error, new_result_text, new_steps).await
}

async fn apply_failed(db: &DatabaseConnection, row: strategy_task::Model, message: String) -> bool {
    apply_phase_logged(
        db,
        row,
        StrategyTaskPhase::Failed,
        Some(message),
        None,
        None,
    )
    .await
}

/// `apply_phase` を呼び、失敗した場合はログを残して `false` にフォールバックする。
async fn apply_phase_logged(
    db: &DatabaseConnection,
    row: strategy_task::Model,
    new_phase: StrategyTaskPhase,
    new_error: Option<String>,
    new_result_text: Option<String>,
    new_steps: Option<serde_json::Value>,
) -> bool {
    match apply_phase(db, row, new_phase, new_error, new_result_text, new_steps).await {
        Ok(updated) => updated,
        Err(err) => {
            tracing::warn!(error = %err, "failed to update strategy_task phase");
            false
        }
    }
}

/// 1 行ぶんの phase / error_summary / result_text 更新と、`strategy_task_step` へのステップ
/// upsert を適用する。前者は差分が無ければ DB 書き込みをしない。主キーと変更カラムのみを
/// `Set` した ActiveModel で UPDATE することで、prompt 等の長文カラムを毎回書き直すのを避ける。
async fn apply_phase(
    db: &DatabaseConnection,
    row: strategy_task::Model,
    new_phase: StrategyTaskPhase,
    new_error: Option<String>,
    new_result_text: Option<String>,
    new_steps: Option<serde_json::Value>,
) -> Result<bool, sea_orm::DbErr> {
    let row_changed = new_phase != row.phase
        || new_error != row.error_summary
        || new_result_text != row.result_text;
    if row_changed {
        let active = strategy_task::ActiveModel {
            task_id: sea_orm::ActiveValue::Unchanged(row.task_id),
            phase: Set(new_phase),
            error_summary: Set(new_error),
            result_text: Set(new_result_text),
            updated_at: Set(Utc::now().fixed_offset()),
            strategy_id: NotSet,
            a2a_task_id: NotSet,
            source: NotSet,
            prompt: NotSet,
            deadline_at: NotSet,
            created_at: NotSet,
            purpose: NotSet,
        };
        strategy_task::Entity::update(active).exec(db).await?;
    }

    let steps_changed = match new_steps {
        Some(serde_json::Value::Array(steps)) if !steps.is_empty() => {
            upsert_steps(db, row.task_id, &steps).await?
        }
        _ => false,
    };

    Ok(row_changed || steps_changed)
}

/// t-rader-agent から届いた実行ステップ配列を `strategy_task_step` へ upsert する。
///
/// `execution_step_id` を主キーとして、既存行と `status`/`output`/`finished_at`/`error` が
/// 全て一致する場合は書き込みをスキップする (`phase_key`/`label`/`model`/`item`/`item_label`/
/// `started_at`/`trace_id`/`span_id` はステップ発行時点で確定し不変のため比較・更新対象外)。
async fn upsert_steps(
    db: &DatabaseConnection,
    task_id: Uuid,
    steps: &[serde_json::Value],
) -> Result<bool, sea_orm::DbErr> {
    let existing: HashMap<Uuid, strategy_task_step::Model> = strategy_task_step::Entity::find()
        .filter(strategy_task_step::Column::TaskId.eq(task_id))
        .all(db)
        .await?
        .into_iter()
        .map(|row| (row.execution_step_id, row))
        .collect();

    let mut to_upsert = Vec::new();
    for raw in steps {
        let wire: StepWire = match serde_json::from_value(raw.clone()) {
            Ok(wire) => wire,
            Err(err) => {
                tracing::warn!(
                    error = %err,
                    task_id = %task_id,
                    "failed to parse strategy task step; skipping",
                );
                continue;
            }
        };
        let Some(status) = step_status_from_raw(&wire.status) else {
            tracing::warn!(
                status = wire.status,
                task_id = %task_id,
                "unknown strategy task step status; skipping",
            );
            continue;
        };

        if let Some(existing_row) = existing.get(&wire.execution_step_id)
            && existing_row.status == status
            && existing_row.output == wire.output
            && existing_row.finished_at == wire.finished_at
            && existing_row.error == wire.error
        {
            continue;
        }

        to_upsert.push(strategy_task_step::ActiveModel {
            execution_step_id: Set(wire.execution_step_id),
            task_id: Set(task_id),
            phase_key: Set(wire.phase_key),
            label: Set(wire.label),
            model: Set(wire.model),
            status: Set(status),
            item: Set(wire.item),
            item_label: Set(wire.item_label),
            output: Set(wire.output),
            started_at: Set(wire.started_at),
            finished_at: Set(wire.finished_at),
            trace_id: Set(wire.trace_id),
            span_id: Set(wire.span_id),
            error: Set(wire.error),
            seq: NotSet,
        });
    }

    if to_upsert.is_empty() {
        return Ok(false);
    }

    strategy_task_step::Entity::insert_many(to_upsert)
        .on_conflict(
            OnConflict::column(strategy_task_step::Column::ExecutionStepId)
                .update_columns([
                    strategy_task_step::Column::Status,
                    strategy_task_step::Column::Output,
                    strategy_task_step::Column::FinishedAt,
                    strategy_task_step::Column::Error,
                ])
                .to_owned(),
        )
        .exec(db)
        .await?;

    Ok(true)
}

/// t-rader-agent から届く実行ステップの wire JSON 形式。`status` は文字列のまま受け取り、
/// `step_status_from_raw` で手動変換する (`AgentTaskState::from_raw` と同じ方針: derive された
/// enum の Serialize/Deserialize が variant 名依存で実際の JSON 値と一致しない可能性があるため
/// 信用しない)。
#[derive(Debug, serde::Deserialize)]
struct StepWire {
    execution_step_id: Uuid,
    phase_key: String,
    label: String,
    model: String,
    status: String,
    #[serde(default)]
    item: Option<serde_json::Value>,
    #[serde(default)]
    item_label: Option<String>,
    #[serde(default)]
    output: Option<serde_json::Value>,
    started_at: DateTime<FixedOffset>,
    #[serde(default)]
    finished_at: Option<DateTime<FixedOffset>>,
    trace_id: String,
    span_id: String,
    #[serde(default)]
    error: Option<String>,
}

fn step_status_from_raw(raw: &str) -> Option<StrategyTaskStepStatus> {
    match raw {
        "running" => Some(StrategyTaskStepStatus::Running),
        "completed" => Some(StrategyTaskStepStatus::Completed),
        "failed" => Some(StrategyTaskStepStatus::Failed),
        _ => None,
    }
}

/// 定期 polling のバックグラウンドタスクを起動する。
///
/// `notify` は webhook 受信時に即時 polling を誘発するための最適化。tick 到来と notify の
/// どちらが先でも 1 回の polling を実行する。
pub fn spawn(
    db: DatabaseConnection,
    agent_client: SharedAgentTaskClient,
    interval: Duration,
    notify: Arc<Notify>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(interval);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        // 起動直後の即時実行は避ける (initial delay)。
        ticker.tick().await;
        loop {
            tokio::select! {
                _ = ticker.tick() => {}
                _ = notify.notified() => {}
            }
            let updated = run_once(&db, &agent_client).await;
            if updated > 0 {
                tracing::info!(updated, "strategy_task phases reconciled");
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use crate::agent_client::{AgentTaskError, FakeAgentTaskClient};
    use crate::entities::strategy;
    use crate::testing::create_test_db;
    use sea_orm::{ActiveModelTrait, ActiveValue::Set};
    use sqlx::PgPool;
    use uuid::Uuid;

    use super::*;

    async fn insert_strategy(db: &DatabaseConnection) -> Uuid {
        let id = Uuid::new_v4();
        strategy::ActiveModel {
            id: Set(id),
            name: Set("test".to_string()),
            description: Set(None),
            sort_order: Set(0),
            created_at: sea_orm::ActiveValue::NotSet,
            updated_at: sea_orm::ActiveValue::NotSet,
            risk_policy: sea_orm::ActiveValue::NotSet,
        }
        .insert(db)
        .await
        .unwrap();
        id
    }

    /// `deadline_offset` だけ現在時刻からずらした deadline_at を持つ行を挿入する。
    /// 過去にすれば「deadline 超過」、未来にすれば「deadline 未到来」の状態を作れる。
    async fn insert_task(
        db: &DatabaseConnection,
        strategy_id: Uuid,
        a2a_task_id: Option<&str>,
        phase: StrategyTaskPhase,
        deadline_offset: chrono::Duration,
    ) -> Uuid {
        let task_id = Uuid::new_v4();
        let now = Utc::now().fixed_offset();
        strategy_task::ActiveModel {
            task_id: Set(task_id),
            strategy_id: Set(strategy_id),
            a2a_task_id: Set(a2a_task_id.map(|s| s.to_string())),
            source: Set("slack".to_string()),
            prompt: Set("hi".to_string()),
            phase: Set(phase),
            error_summary: Set(None),
            result_text: Set(None),
            deadline_at: Set(now + deadline_offset),
            purpose: NotSet,
            created_at: NotSet,
            updated_at: NotSet,
        }
        .insert(db)
        .await
        .unwrap();
        task_id
    }

    async fn fetch_task(db: &DatabaseConnection, task_id: Uuid) -> strategy_task::Model {
        strategy_task::Entity::find_by_id(task_id)
            .one(db)
            .await
            .unwrap()
            .unwrap()
    }

    async fn fetch_steps(db: &DatabaseConnection, task_id: Uuid) -> Vec<strategy_task_step::Model> {
        strategy_task_step::Entity::find()
            .filter(strategy_task_step::Column::TaskId.eq(task_id))
            .order_by_asc(strategy_task_step::Column::Seq)
            .all(db)
            .await
            .unwrap()
    }

    const FAR_FUTURE: chrono::Duration = chrono::Duration::minutes(15);
    const PAST: chrono::Duration = chrono::Duration::seconds(-1);

    #[sqlx::test(migrations = false)]
    async fn reconciles_completed_running_and_failed_states(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_id = insert_strategy(&db).await;
        let completed_id = insert_task(
            &db,
            strategy_id,
            Some("t-completed"),
            StrategyTaskPhase::Running,
            FAR_FUTURE,
        )
        .await;
        let failed_id = insert_task(
            &db,
            strategy_id,
            Some("t-failed"),
            StrategyTaskPhase::Running,
            FAR_FUTURE,
        )
        .await;
        let running_id = insert_task(
            &db,
            strategy_id,
            Some("t-running"),
            StrategyTaskPhase::Running,
            FAR_FUTURE,
        )
        .await;

        let fake = Arc::new(FakeAgentTaskClient::new());
        fake.set_status(
            "t-completed",
            AgentTaskStatus {
                state: AgentTaskState::Completed,
                result_text: Some("all good".to_string()),
                error_kind: None,
                steps: None,
            },
        )
        .await;
        fake.set_status(
            "t-failed",
            AgentTaskStatus {
                state: AgentTaskState::Failed,
                result_text: None,
                error_kind: Some("usage_limit".to_string()),
                steps: None,
            },
        )
        .await;
        fake.set_status(
            "t-running",
            AgentTaskStatus {
                state: AgentTaskState::Working,
                result_text: None,
                error_kind: None,
                steps: None,
            },
        )
        .await;

        let agent_client: SharedAgentTaskClient = fake.clone();
        let updated = run_once(&db, &agent_client).await;
        // running は phase (Running) も error/result も変化しないので更新カウントに含まれない。
        assert_eq!(updated, 2);

        let completed = fetch_task(&db, completed_id).await;
        let failed = fetch_task(&db, failed_id).await;
        let running = fetch_task(&db, running_id).await;

        assert_eq!(
            (
                completed.phase,
                completed.result_text,
                completed.error_summary,
            ),
            (
                StrategyTaskPhase::Completed,
                Some("all good".to_string()),
                None,
            ),
        );
        assert_eq!(
            (failed.phase, failed.result_text, failed.error_summary),
            (
                StrategyTaskPhase::Failed,
                None,
                Some("usage_limit".to_string()),
            ),
        );
        assert_eq!(
            (running.phase, running.result_text, running.error_summary),
            (StrategyTaskPhase::Running, None, None),
        );
    }

    #[sqlx::test(migrations = false)]
    async fn input_required_maps_to_failed(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_id = insert_strategy(&db).await;
        let task_id = insert_task(
            &db,
            strategy_id,
            Some("t-ir"),
            StrategyTaskPhase::Running,
            FAR_FUTURE,
        )
        .await;

        let fake = Arc::new(FakeAgentTaskClient::new());
        fake.set_status(
            "t-ir",
            AgentTaskStatus {
                state: AgentTaskState::InputRequired,
                result_text: None,
                error_kind: None,
                steps: None,
            },
        )
        .await;
        let agent_client: SharedAgentTaskClient = fake.clone();

        let updated = run_once(&db, &agent_client).await;
        assert_eq!(updated, 1);

        let row = fetch_task(&db, task_id).await;
        assert_eq!(
            (row.phase, row.error_summary),
            (
                StrategyTaskPhase::Failed,
                Some("agent task failed".to_string())
            ),
        );
    }

    #[sqlx::test(migrations = false)]
    async fn not_found_after_deadline_marks_failed(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_id = insert_strategy(&db).await;
        let task_id = insert_task(
            &db,
            strategy_id,
            Some("ghost"),
            StrategyTaskPhase::Running,
            PAST,
        )
        .await;
        let fake: SharedAgentTaskClient = Arc::new(FakeAgentTaskClient::new());

        let updated = run_once(&db, &fake).await;
        assert_eq!(updated, 1);

        let row = fetch_task(&db, task_id).await;
        assert_eq!(row.phase, StrategyTaskPhase::Failed);
        assert_eq!(
            row.error_summary,
            Some("agent task ghost not found".to_string())
        );
    }

    #[sqlx::test(migrations = false)]
    async fn not_found_before_deadline_is_skipped(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_id = insert_strategy(&db).await;
        let task_id = insert_task(
            &db,
            strategy_id,
            Some("fresh"),
            StrategyTaskPhase::Running,
            FAR_FUTURE,
        )
        .await;
        let fake: SharedAgentTaskClient = Arc::new(FakeAgentTaskClient::new());

        let updated = run_once(&db, &fake).await;
        assert_eq!(updated, 0);

        let row = fetch_task(&db, task_id).await;
        assert_eq!(row.phase, StrategyTaskPhase::Running);
        assert_eq!(row.error_summary, None);
    }

    #[sqlx::test(migrations = false)]
    async fn orphaned_row_without_a2a_task_id_failed_after_deadline(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_id = insert_strategy(&db).await;
        let task_id = insert_task(&db, strategy_id, None, StrategyTaskPhase::Pending, PAST).await;
        let fake: SharedAgentTaskClient = Arc::new(FakeAgentTaskClient::new());

        let updated = run_once(&db, &fake).await;
        assert_eq!(updated, 1);

        let row = fetch_task(&db, task_id).await;
        assert_eq!(row.phase, StrategyTaskPhase::Failed);
        assert_eq!(
            row.error_summary,
            Some("agent task submission was not recorded".to_string())
        );
    }

    #[sqlx::test(migrations = false)]
    async fn orphaned_row_without_a2a_task_id_skipped_before_deadline(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_id = insert_strategy(&db).await;
        let task_id = insert_task(
            &db,
            strategy_id,
            None,
            StrategyTaskPhase::Pending,
            FAR_FUTURE,
        )
        .await;
        let fake: SharedAgentTaskClient = Arc::new(FakeAgentTaskClient::new());

        let updated = run_once(&db, &fake).await;
        assert_eq!(updated, 0);

        let row = fetch_task(&db, task_id).await;
        assert_eq!(row.phase, StrategyTaskPhase::Pending);
    }

    #[sqlx::test(migrations = false)]
    async fn transient_error_after_deadline_marks_failed(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_id = insert_strategy(&db).await;
        let task_id = insert_task(
            &db,
            strategy_id,
            Some("flaky"),
            StrategyTaskPhase::Running,
            PAST,
        )
        .await;
        let fake = Arc::new(FakeAgentTaskClient::new());
        fake.set_get_error(AgentTaskError::Network("connection refused".to_string()))
            .await;
        let agent_client: SharedAgentTaskClient = fake;

        let updated = run_once(&db, &agent_client).await;
        assert_eq!(updated, 1);

        let row = fetch_task(&db, task_id).await;
        assert_eq!(row.phase, StrategyTaskPhase::Failed);
    }

    #[sqlx::test(migrations = false)]
    async fn transient_error_before_deadline_is_skipped(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_id = insert_strategy(&db).await;
        let task_id = insert_task(
            &db,
            strategy_id,
            Some("flaky"),
            StrategyTaskPhase::Running,
            FAR_FUTURE,
        )
        .await;
        let fake = Arc::new(FakeAgentTaskClient::new());
        fake.set_get_error(AgentTaskError::Network("connection refused".to_string()))
            .await;
        let agent_client: SharedAgentTaskClient = fake;

        let updated = run_once(&db, &agent_client).await;
        assert_eq!(updated, 0);

        let row = fetch_task(&db, task_id).await;
        assert_eq!(row.phase, StrategyTaskPhase::Running);
    }

    #[sqlx::test(migrations = false)]
    async fn apply_phase_upserts_steps_and_skips_unchanged(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_id = insert_strategy(&db).await;
        let task_id = insert_task(
            &db,
            strategy_id,
            Some("t-steps"),
            StrategyTaskPhase::Running,
            FAR_FUTURE,
        )
        .await;

        let step_id = Uuid::new_v4();
        let running_step = serde_json::json!({
            "execution_step_id": step_id,
            "phase_key": "investigate",
            "label": "仮説の調査",
            "model": "test-model",
            "status": "running",
            "started_at": "2026-01-01T00:00:00.000Z",
            "trace_id": "trace-1",
            "span_id": "span-1",
        });

        // 新規ステップは insert される。
        let row = fetch_task(&db, task_id).await;
        let updated = apply_phase(
            &db,
            row,
            StrategyTaskPhase::Running,
            None,
            None,
            Some(serde_json::json!([running_step.clone()])),
        )
        .await
        .unwrap();
        assert!(updated);

        let steps = fetch_steps(&db, task_id).await;
        assert_eq!(steps.len(), 1);
        let seq = steps[0].seq;
        assert_eq!(
            steps[0],
            strategy_task_step::Model {
                execution_step_id: step_id,
                task_id,
                phase_key: "investigate".to_string(),
                label: "仮説の調査".to_string(),
                model: "test-model".to_string(),
                status: StrategyTaskStepStatus::Running,
                item: None,
                item_label: None,
                output: None,
                started_at: DateTime::parse_from_rfc3339("2026-01-01T00:00:00.000Z").unwrap(),
                finished_at: None,
                trace_id: "trace-1".to_string(),
                span_id: "span-1".to_string(),
                error: None,
                seq,
            },
        );

        // 同じ内容の再送は upsert 対象にならない。
        let row = fetch_task(&db, task_id).await;
        let updated = apply_phase(
            &db,
            row,
            StrategyTaskPhase::Running,
            None,
            None,
            Some(serde_json::json!([running_step])),
        )
        .await
        .unwrap();
        assert!(!updated);
        assert_eq!(fetch_steps(&db, task_id).await, vec![steps[0].clone()]);

        // status/output の変化は既存行 (同じ execution_step_id) を更新する。
        let completed_step = serde_json::json!({
            "execution_step_id": step_id,
            "phase_key": "investigate",
            "label": "仮説の調査",
            "model": "test-model",
            "status": "completed",
            "output": {"summary": "ok"},
            "started_at": "2026-01-01T00:00:00.000Z",
            "finished_at": "2026-01-01T00:00:05.000Z",
            "trace_id": "trace-1",
            "span_id": "span-1",
        });
        let row = fetch_task(&db, task_id).await;
        let updated = apply_phase(
            &db,
            row,
            StrategyTaskPhase::Running,
            None,
            None,
            Some(serde_json::json!([completed_step])),
        )
        .await
        .unwrap();
        assert!(updated);

        let steps = fetch_steps(&db, task_id).await;
        assert_eq!(steps.len(), 1);
        assert_eq!(
            steps[0],
            strategy_task_step::Model {
                execution_step_id: step_id,
                task_id,
                phase_key: "investigate".to_string(),
                label: "仮説の調査".to_string(),
                model: "test-model".to_string(),
                status: StrategyTaskStepStatus::Completed,
                item: None,
                item_label: None,
                output: Some(serde_json::json!({"summary": "ok"})),
                started_at: DateTime::parse_from_rfc3339("2026-01-01T00:00:00.000Z").unwrap(),
                finished_at: Some(
                    DateTime::parse_from_rfc3339("2026-01-01T00:00:05.000Z").unwrap()
                ),
                trace_id: "trace-1".to_string(),
                span_id: "span-1".to_string(),
                error: None,
                seq,
            },
        );
    }
}
