//! 戦略タスクの phase 監視 (polling)
//!
//! 一定 interval で `strategy_task.phase IN ('pending', 'running')` の行を LIST し、
//! t-rader-agent の内部 API (`GET /internal/tasks/:task_id`) を照会して結果を DB に反映する。
//! t-rader-agent からの webhook 受信は `notify` 経由で polling を即時発火させるための最適化に
//! 過ぎず、決着の正 (最終的な整合性を保証する経路) は本 polling である。
//!
//! `deadline_at` を過ぎても決着しない行 (内部 API 到達不能、投入自体の記録漏れを含む) は
//! failed に確定し、沈黙したまま残ることを防ぐ。

use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, FixedOffset, Utc};
use sea_orm::ActiveValue::{NotSet, Set};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder};
use tokio::sync::{Notify, Semaphore};

use crate::agent_client::{AgentTaskError, AgentTaskState, AgentTaskStatus, SharedAgentTaskClient};
use crate::entities::sea_orm_active_enums::StrategyTaskPhase;
use crate::entities::strategy_task;

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
    apply_phase_logged(db, row, new_phase, new_error, new_result_text).await
}

async fn apply_failed(db: &DatabaseConnection, row: strategy_task::Model, message: String) -> bool {
    apply_phase_logged(db, row, StrategyTaskPhase::Failed, Some(message), None).await
}

/// `apply_phase` を呼び、失敗した場合はログを残して `false` にフォールバックする。
async fn apply_phase_logged(
    db: &DatabaseConnection,
    row: strategy_task::Model,
    new_phase: StrategyTaskPhase,
    new_error: Option<String>,
    new_result_text: Option<String>,
) -> bool {
    match apply_phase(db, row, new_phase, new_error, new_result_text).await {
        Ok(updated) => updated,
        Err(err) => {
            tracing::warn!(error = %err, "failed to update strategy_task phase");
            false
        }
    }
}

/// 1 行ぶんの phase / error_summary / result_text 更新を適用する。差分が無ければ DB 書き込みを
/// しない。主キーと変更カラムのみを `Set` した ActiveModel で UPDATE することで、prompt 等の
/// 長文カラムを毎回書き直すのを避ける。
async fn apply_phase(
    db: &DatabaseConnection,
    row: strategy_task::Model,
    new_phase: StrategyTaskPhase,
    new_error: Option<String>,
    new_result_text: Option<String>,
) -> Result<bool, sea_orm::DbErr> {
    if new_phase == row.phase
        && new_error == row.error_summary
        && new_result_text == row.result_text
    {
        return Ok(false);
    }
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
    };
    strategy_task::Entity::update(active).exec(db).await?;
    Ok(true)
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
            agents_md: sea_orm::ActiveValue::NotSet,
            skills: sea_orm::ActiveValue::NotSet,
            agent_status: sea_orm::ActiveValue::NotSet,
            agent_error: sea_orm::ActiveValue::NotSet,
            created_at: sea_orm::ActiveValue::NotSet,
            updated_at: sea_orm::ActiveValue::NotSet,
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
            },
        )
        .await;
        fake.set_status(
            "t-failed",
            AgentTaskStatus {
                state: AgentTaskState::Failed,
                result_text: None,
                error_kind: Some("usage_limit".to_string()),
            },
        )
        .await;
        fake.set_status(
            "t-running",
            AgentTaskStatus {
                state: AgentTaskState::Working,
                result_text: None,
                error_kind: None,
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
                completed.error_summary
            ),
            (
                StrategyTaskPhase::Completed,
                Some("all good".to_string()),
                None
            ),
        );
        assert_eq!(
            (failed.phase, failed.result_text, failed.error_summary),
            (
                StrategyTaskPhase::Failed,
                None,
                Some("usage_limit".to_string())
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
}
