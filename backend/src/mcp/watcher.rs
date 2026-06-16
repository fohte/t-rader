//! 戦略タスクの phase 監視 (polling)
//!
//! 一定 interval で `strategy_task.phase IN ('pending', 'running')` の行を LIST し、
//! kubeopencode の Task CR を get_task_status で照会して結果を DB に反映する。
//!
//! 監視期間中に Task CR 自体が消えた場合は、最後の phase / error_summary を「失敗 (lost)」
//! として確定させ、以降の polling 対象から外す。

use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use sea_orm::ActiveValue::{NotSet, Set};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder};
use tokio::sync::Semaphore;

use crate::entities::sea_orm_active_enums::StrategyTaskPhase;
use crate::entities::strategy_task;
use crate::kubeopencode::{KubeopencodeError, SharedKubeopencodeClient, TaskPhase};

pub const DEFAULT_INTERVAL: Duration = Duration::from_secs(10);

/// 行 insert 直後 — まだ Task CR が apiserver に登録されていない race window — で
/// NotFound を Failed に確定させないための猶予期間。これ未満の経過時間で NotFound を
/// 受け取った場合は次の tick まで判定を保留する。
pub const NOT_FOUND_GRACE: chrono::Duration = chrono::Duration::seconds(60);

/// 1 tick あたりに同時実行する kubeopencode API 問い合わせ数の上限。1 件の遅延・タイムアウトが
/// 他の reconcile をブロックしないよう、行ごとに `get_task_status` を並列化する。kube-apiserver に
/// 同時接続を投げすぎないよう上限を設ける。
const MAX_CONCURRENT_STATUS_FETCHES: usize = 8;

/// 1 回分の polling を実行する。失敗した個別 task はログに残し、他の task の処理を継続する。
///
/// 戻り値は phase 更新が走った task 数。
pub async fn run_once(db: &DatabaseConnection, kube: &SharedKubeopencodeClient) -> usize {
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

    let semaphore = Arc::new(Semaphore::new(MAX_CONCURRENT_STATUS_FETCHES));
    let mut handles = Vec::with_capacity(rows.len());
    for row in rows {
        let kube = kube.clone();
        let db = db.clone();
        let sem = semaphore.clone();
        handles.push(tokio::spawn(async move {
            // semaphore で kube-apiserver への同時接続数を制限する。
            let _permit = match sem.acquire_owned().await {
                Ok(permit) => permit,
                Err(_) => return false,
            };
            reconcile_one(&db, &kube, row).await
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
    kube: &SharedKubeopencodeClient,
    row: strategy_task::Model,
) -> bool {
    let (new_phase, new_error) = match kube.get_task_status(&row.kubeopencode_task_name).await {
        Ok(status) => {
            let phase = match status.phase {
                Some(TaskPhase::Pending) => StrategyTaskPhase::Pending,
                Some(TaskPhase::Running) => StrategyTaskPhase::Running,
                Some(TaskPhase::Completed) => StrategyTaskPhase::Completed,
                Some(TaskPhase::Failed) => StrategyTaskPhase::Failed,
                None => row.phase.clone(),
            };
            let error = match phase {
                StrategyTaskPhase::Failed => {
                    Some(status.message.unwrap_or_else(|| "task failed".to_string()))
                }
                _ => row.error_summary.clone(),
            };
            (phase, error)
        }
        Err(KubeopencodeError::NotFound(name)) => {
            // CR insert と create_task は別段階で実行されるため、insert 直後 ~ create 完了前の
            // 窓で NotFound が返り得る。本物の消失と誤検知を区別するため、行が
            // `NOT_FOUND_GRACE` 未満なら判定を保留する。
            let age = Utc::now().fixed_offset() - row.created_at;
            if age < NOT_FOUND_GRACE {
                tracing::debug!(
                    task = %name,
                    age_ms = age.num_milliseconds(),
                    "task cr not found within grace period; will retry"
                );
                return false;
            }
            (
                StrategyTaskPhase::Failed,
                Some(format!("task cr {name} not found")),
            )
        }
        Err(err) => {
            tracing::warn!(
                error = %err,
                task = %row.kubeopencode_task_name,
                "failed to fetch task status; will retry on next tick",
            );
            return false;
        }
    };
    match apply_phase(db, row, new_phase, new_error).await {
        Ok(updated) => updated,
        Err(err) => {
            tracing::warn!(error = %err, "failed to update strategy_task phase");
            false
        }
    }
}

/// 1 行ぶんの phase / error_summary 更新を適用する。差分が無ければ DB 書き込みをしない。
/// 主キーと変更カラムのみを `Set` した ActiveModel で UPDATE することで、prompt 等の
/// 長文カラムを毎回書き直すのを避ける。
async fn apply_phase(
    db: &DatabaseConnection,
    row: strategy_task::Model,
    new_phase: StrategyTaskPhase,
    new_error: Option<String>,
) -> Result<bool, sea_orm::DbErr> {
    if new_phase == row.phase && new_error == row.error_summary {
        return Ok(false);
    }
    let active = strategy_task::ActiveModel {
        task_id: sea_orm::ActiveValue::Unchanged(row.task_id),
        phase: Set(new_phase),
        error_summary: Set(new_error),
        updated_at: Set(Utc::now().fixed_offset()),
        strategy_id: NotSet,
        kubeopencode_task_name: NotSet,
        source: NotSet,
        prompt: NotSet,
        created_at: NotSet,
    };
    strategy_task::Entity::update(active).exec(db).await?;
    Ok(true)
}

/// 定期 polling のバックグラウンドタスクを起動する。
pub fn spawn(
    db: DatabaseConnection,
    kube: SharedKubeopencodeClient,
    interval: Duration,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(interval);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        // 起動直後の即時実行は避ける (initial delay)。
        ticker.tick().await;
        loop {
            ticker.tick().await;
            let updated = run_once(&db, &kube).await;
            if updated > 0 {
                tracing::info!(updated, "strategy_task phases reconciled");
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use crate::entities::strategy;
    use crate::kubeopencode::{FakeKubeopencodeClient, TaskCrStatus};
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
        }
        .insert(db)
        .await
        .unwrap();
        id
    }

    async fn insert_pending_task(db: &DatabaseConnection, strategy_id: Uuid, name: &str) -> Uuid {
        insert_pending_task_aged(db, strategy_id, name, chrono::Duration::seconds(0)).await
    }

    /// `aged` だけ過去に created_at を寄せた pending タスクを挿入する。grace period を
    /// 跨いだ NotFound 判定のテスト用。
    async fn insert_pending_task_aged(
        db: &DatabaseConnection,
        strategy_id: Uuid,
        name: &str,
        aged: chrono::Duration,
    ) -> Uuid {
        let task_id = Uuid::new_v4();
        let backdated = Utc::now().fixed_offset() - aged;
        strategy_task::ActiveModel {
            task_id: Set(task_id),
            strategy_id: Set(strategy_id),
            kubeopencode_task_name: Set(name.to_string()),
            source: Set("slack".to_string()),
            prompt: Set("hi".to_string()),
            phase: Set(StrategyTaskPhase::Pending),
            error_summary: Set(None),
            created_at: Set(backdated),
            updated_at: Set(backdated),
        }
        .insert(db)
        .await
        .unwrap();
        task_id
    }

    #[sqlx::test(migrations = false)]
    async fn reconciles_completed_and_failed(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_id = insert_strategy(&db).await;
        let _ = insert_pending_task(&db, strategy_id, "task-completed").await;
        let _ = insert_pending_task(&db, strategy_id, "task-failed").await;
        let _ = insert_pending_task(&db, strategy_id, "task-running").await;

        let fake = Arc::new(FakeKubeopencodeClient::new());
        fake.set_status(
            "task-completed",
            TaskCrStatus {
                phase: Some(TaskPhase::Completed),
                message: None,
            },
        )
        .await;
        fake.set_status(
            "task-failed",
            TaskCrStatus {
                phase: Some(TaskPhase::Failed),
                message: Some("boom".into()),
            },
        )
        .await;
        fake.set_status(
            "task-running",
            TaskCrStatus {
                phase: Some(TaskPhase::Running),
                message: None,
            },
        )
        .await;

        let kube: SharedKubeopencodeClient = fake.clone();
        let updated = run_once(&db, &kube).await;
        assert_eq!(updated, 3);

        let rows = strategy_task::Entity::find()
            .order_by_asc(strategy_task::Column::KubeopencodeTaskName)
            .all(&db)
            .await
            .unwrap();
        let mut summary: Vec<(String, StrategyTaskPhase, Option<String>)> = rows
            .into_iter()
            .map(|r| (r.kubeopencode_task_name, r.phase, r.error_summary))
            .collect();
        summary.sort_by(|a, b| a.0.cmp(&b.0));

        assert_eq!(
            summary,
            vec![
                (
                    "task-completed".to_string(),
                    StrategyTaskPhase::Completed,
                    None
                ),
                (
                    "task-failed".to_string(),
                    StrategyTaskPhase::Failed,
                    Some("boom".to_string()),
                ),
                ("task-running".to_string(), StrategyTaskPhase::Running, None),
            ],
        );
    }

    #[sqlx::test(migrations = false)]
    async fn marks_missing_task_as_failed(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_id = insert_strategy(&db).await;
        let _ = insert_pending_task_aged(
            &db,
            strategy_id,
            "ghost",
            NOT_FOUND_GRACE + chrono::Duration::seconds(1),
        )
        .await;
        let fake: SharedKubeopencodeClient = Arc::new(FakeKubeopencodeClient::new());

        let updated = run_once(&db, &fake).await;
        assert_eq!(updated, 1);

        let row = strategy_task::Entity::find()
            .one(&db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(row.phase, StrategyTaskPhase::Failed);
        assert_eq!(
            row.error_summary,
            Some("task cr ghost not found".to_string())
        );
    }

    #[sqlx::test(migrations = false)]
    async fn skips_recent_not_found_within_grace(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_id = insert_strategy(&db).await;
        let _ = insert_pending_task(&db, strategy_id, "fresh").await;
        let fake: SharedKubeopencodeClient = Arc::new(FakeKubeopencodeClient::new());

        let updated = run_once(&db, &fake).await;
        assert_eq!(updated, 0);

        let row = strategy_task::Entity::find()
            .one(&db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(row.phase, StrategyTaskPhase::Pending);
        assert_eq!(row.error_summary, None);
    }
}
