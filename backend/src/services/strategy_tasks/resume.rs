use sea_orm::ActiveModelTrait;
use sea_orm::ActiveValue::Set;
use sea_orm::sea_query::{Expr, ExprTrait};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder};
use uuid::Uuid;

use super::{DEADLINE_DURATION, SubmittedTask, phase_str, step_to_wire_json};
use crate::agent_client::{AgentTaskError, SharedAgentTaskClient, SubmitAgentTask};
use crate::entities::sea_orm_active_enums::StrategyTaskPhase;
use crate::entities::{strategy_task, strategy_task_step};

#[derive(Debug, thiserror::Error)]
pub enum ResumeTaskError {
    #[error("strategy task {0} not found")]
    NotFound(Uuid),
    #[error("strategy task {0} is not failed (current phase: {1})")]
    NotFailed(Uuid, &'static str),
    #[error(transparent)]
    Database(#[from] sea_orm::DbErr),
    #[error(transparent)]
    AgentTask(#[from] AgentTaskError),
}

/// failed な戦略タスクを、成功済みステップを再実行せずに同じ行のまま再開する。
///
/// 新しい strategy_task 行は作らない (a2a_task_id だけ差し替え、phase を Running に戻す)。
/// 全 strategy_task_step 行 (completed/failed/running 問わず) を `resume_steps` として
/// agent に渡し、どのステップをスキップするか (status=completed のみ) は agent 側の判断に
/// 委ねる — backend は中身を解釈しない。
pub async fn resume_task(
    db: &DatabaseConnection,
    agent_client: &SharedAgentTaskClient,
    task_id: Uuid,
) -> Result<SubmittedTask, ResumeTaskError> {
    let row = strategy_task::Entity::find_by_id(task_id)
        .one(db)
        .await?
        .ok_or(ResumeTaskError::NotFound(task_id))?;
    if row.phase != StrategyTaskPhase::Failed {
        return Err(ResumeTaskError::NotFailed(task_id, phase_str(&row.phase)));
    }

    // 「Failed である」ことの確認と「Running に倒す」ことを 1 回の条件付き UPDATE
    // で原子化する。上の事前チェックだけでは、同じ task_id への並行呼び出しが
    // 両方とも Failed を読んでしまい二重に agent へ投入されうる。
    let claimed = strategy_task::Entity::update_many()
        .col_expr(
            strategy_task::Column::Phase,
            Expr::value(StrategyTaskPhase::Running),
        )
        .col_expr(
            strategy_task::Column::UpdatedAt,
            Expr::value(chrono::Utc::now().fixed_offset()),
        )
        .filter(
            strategy_task::Column::TaskId
                .eq(task_id)
                .and(strategy_task::Column::Phase.eq(StrategyTaskPhase::Failed)),
        )
        .exec(db)
        .await?;
    if claimed.rows_affected == 0 {
        return Err(ResumeTaskError::NotFailed(task_id, "failed"));
    }

    let step_rows = strategy_task_step::Entity::find()
        .filter(strategy_task_step::Column::TaskId.eq(task_id))
        .order_by_asc(strategy_task_step::Column::Seq)
        .all(db)
        .await?;
    let resume_steps = step_rows
        .into_iter()
        .map(step_to_resume_wire_json)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| {
            ResumeTaskError::Database(sea_orm::DbErr::Custom(format!(
                "failed to serialize strategy_task_step: {err}"
            )))
        })?;

    let agent_ref = match agent_client
        .submit(SubmitAgentTask {
            strategy_id: row.strategy_id,
            prompt: row.prompt.clone(),
            purpose: row.purpose.clone(),
            resume_steps: (!resume_steps.is_empty()).then_some(resume_steps),
        })
        .await
    {
        Ok(agent_ref) => agent_ref,
        Err(err) => {
            tracing::warn!(
                error = %err,
                task_id = %task_id,
                "agent task resume submission failed",
            );
            let failed = strategy_task::ActiveModel {
                task_id: Set(task_id),
                phase: Set(StrategyTaskPhase::Failed),
                error_summary: Set(Some(format!("agent task resume submission failed: {err}"))),
                updated_at: Set(chrono::Utc::now().fixed_offset()),
                ..Default::default()
            };
            if let Err(update_err) = failed.update(db).await {
                // 直前の claim で phase は既に Running へ進んでいるため、この
                // 更新が失敗すると行は Failed に戻せないまま Running で宙に浮く。
                tracing::error!(
                    error = %update_err,
                    task_id = %task_id,
                    "failed to record resume submission failure on strategy_task",
                );
            }
            return Err(ResumeTaskError::AgentTask(err));
        }
    };

    let now = chrono::Utc::now().fixed_offset();
    let running = strategy_task::ActiveModel {
        task_id: Set(task_id),
        a2a_task_id: Set(Some(agent_ref.task_id.clone())),
        error_summary: Set(None),
        result_text: Set(None),
        deadline_at: Set(now + DEADLINE_DURATION),
        updated_at: Set(now),
        ..Default::default()
    };
    if let Err(err) = running.update(db).await {
        // claim で phase は既に Running へ進んでいるため、この行は a2a_task_id が
        // 記録されないまま孤児化する。watcher が deadline 超過で failed 確定するが、
        // 実際には agent 側でタスクが動いているので、追跡できるよう a2a_task_id を
        // ログに残す。
        tracing::error!(
            error = %err,
            task_id = %task_id,
            a2a_task_id = %agent_ref.task_id,
            "resumed strategy task submitted but failed to record a2a_task_id; row orphaned until deadline",
        );
        return Err(ResumeTaskError::Database(err));
    }

    Ok(SubmittedTask {
        task_id,
        a2a_task_id: agent_ref.task_id,
    })
}

/// `POST /internal/tasks` の `resume_steps` 配列 1 要素分の wire JSON 形状。
/// `step_to_wire_json` の出力に `execution_step_id` を足したもの: agent が再実行時に
/// 同じ id を使い回すことで MCP tool 呼び出しの `x-execution-id` を安定させ、ノート書き込み
/// (execution_id で既存ノートを探して更新する) の重複を防ぐ契約のため。
fn step_to_resume_wire_json(
    row: strategy_task_step::Model,
) -> Result<serde_json::Value, serde_json::Error> {
    let execution_step_id = row.execution_step_id;
    let mut value = step_to_wire_json(row)?;
    if let serde_json::Value::Object(map) = &mut value {
        map.insert(
            "execution_step_id".into(),
            serde_json::json!(execution_step_id),
        );
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use sqlx::PgPool;

    use super::super::TaskSource;
    use super::*;
    use crate::agent_client::FakeAgentTaskClient;
    use crate::entities::sea_orm_active_enums::StrategyTaskStepStatus;
    use crate::testing::{create_test_db, insert_test_strategy};

    async fn insert_task_with_phase(
        db: &DatabaseConnection,
        strategy_id: Uuid,
        phase: StrategyTaskPhase,
        prompt: &str,
        purpose: Option<&str>,
    ) -> Uuid {
        let task_id = Uuid::new_v4();
        let now = chrono::Utc::now().fixed_offset();
        strategy_task::ActiveModel {
            task_id: Set(task_id),
            strategy_id: Set(strategy_id),
            a2a_task_id: Set(None),
            source: Set(TaskSource::Review.as_str().to_string()),
            prompt: Set(prompt.to_string()),
            phase: Set(phase),
            error_summary: Set(Some("boom".to_string())),
            result_text: Set(Some("stale result".to_string())),
            deadline_at: Set(now),
            purpose: Set(purpose.map(str::to_string)),
            as_of: Set(Some(now)),
            created_at: Set(now),
            updated_at: Set(now),
        }
        .insert(db)
        .await
        .expect("insert test strategy_task");
        task_id
    }

    async fn insert_task_step(
        db: &DatabaseConnection,
        task_id: Uuid,
        execution_step_id: Uuid,
        phase_key: &str,
        status: StrategyTaskStepStatus,
        seq: i64,
    ) {
        strategy_task_step::ActiveModel {
            execution_step_id: Set(execution_step_id),
            task_id: Set(task_id),
            phase_key: Set(phase_key.to_string()),
            label: Set(phase_key.to_string()),
            model: Set("m".to_string()),
            status: Set(status),
            item: Set(None),
            item_label: Set(None),
            output: Set(None),
            started_at: Set(chrono::DateTime::parse_from_rfc3339("2026-01-01T00:00:00Z").unwrap()),
            finished_at: Set(None),
            trace_id: Set("trace-1".to_string()),
            span_id: Set("span-1".to_string()),
            error: Set(None),
            seq: Set(seq),
        }
        .insert(db)
        .await
        .expect("insert test strategy_task_step");
    }

    #[sqlx::test(migrations = false)]
    async fn resume_task_rejects_when_not_failed(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_id = insert_test_strategy(&db, "s").await;
        let task_id =
            insert_task_with_phase(&db, strategy_id, StrategyTaskPhase::Running, "p", None).await;
        let fake = Arc::new(FakeAgentTaskClient::new());
        let agent_client: SharedAgentTaskClient = fake.clone();

        let err = resume_task(&db, &agent_client, task_id)
            .await
            .expect_err("running task must not be resumable");
        assert!(
            matches!(&err, ResumeTaskError::NotFailed(id, phase) if *id == task_id && *phase == "running")
        );
        assert!(fake.submitted.lock().await.is_empty());
    }

    #[sqlx::test(migrations = false)]
    async fn resume_task_not_found(pool: PgPool) {
        let db = create_test_db(pool).await;
        let agent_client: SharedAgentTaskClient = Arc::new(FakeAgentTaskClient::new());
        let task_id = Uuid::new_v4();

        let err = resume_task(&db, &agent_client, task_id)
            .await
            .expect_err("missing task should fail");
        assert!(matches!(err, ResumeTaskError::NotFound(id) if id == task_id));
    }

    #[sqlx::test(migrations = false)]
    async fn resume_task_resubmits_all_steps_and_updates_the_row_in_place(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_id = insert_test_strategy(&db, "s").await;
        let task_id = insert_task_with_phase(
            &db,
            strategy_id,
            StrategyTaskPhase::Failed,
            "prompt text",
            Some("explore"),
        )
        .await;
        let completed_step_id = Uuid::new_v4();
        let failed_step_id = Uuid::new_v4();
        insert_task_step(
            &db,
            task_id,
            completed_step_id,
            "plan",
            StrategyTaskStepStatus::Completed,
            1,
        )
        .await;
        insert_task_step(
            &db,
            task_id,
            failed_step_id,
            "investigate",
            StrategyTaskStepStatus::Failed,
            2,
        )
        .await;
        let fake = Arc::new(FakeAgentTaskClient::new());
        fake.set_next_task_id("agent-task-resumed").await;
        let agent_client: SharedAgentTaskClient = fake.clone();
        let started_at = chrono::DateTime::parse_from_rfc3339("2026-01-01T00:00:00Z")
            .unwrap()
            .to_rfc3339();

        let submitted = resume_task(&db, &agent_client, task_id)
            .await
            .expect("resume ok");

        let submitted_resume_steps = fake
            .submitted
            .lock()
            .await
            .first()
            .expect("submit called once")
            .resume_steps
            .clone()
            .expect("resume_steps present");
        let row = strategy_task::Entity::find_by_id(task_id)
            .one(&db)
            .await
            .unwrap()
            .expect("row still exists");

        #[derive(Debug, PartialEq)]
        struct ResumeOutcome {
            submitted_a2a_task_id: String,
            resume_steps: Vec<serde_json::Value>,
            row_a2a_task_id: Option<String>,
            row_phase: StrategyTaskPhase,
            row_error_summary: Option<String>,
            row_result_text: Option<String>,
        }

        assert_eq!(
            ResumeOutcome {
                submitted_a2a_task_id: submitted.a2a_task_id,
                resume_steps: submitted_resume_steps,
                row_a2a_task_id: row.a2a_task_id,
                row_phase: row.phase,
                row_error_summary: row.error_summary,
                row_result_text: row.result_text,
            },
            ResumeOutcome {
                submitted_a2a_task_id: "agent-task-resumed".to_string(),
                resume_steps: vec![
                    serde_json::json!({
                        "execution_step_id": completed_step_id,
                        "phase_key": "plan",
                        "label": "plan",
                        "model": "m",
                        "status": "completed",
                        "started_at": started_at,
                        "trace_id": "trace-1",
                        "span_id": "span-1",
                    }),
                    serde_json::json!({
                        "execution_step_id": failed_step_id,
                        "phase_key": "investigate",
                        "label": "investigate",
                        "model": "m",
                        "status": "failed",
                        "started_at": started_at,
                        "trace_id": "trace-1",
                        "span_id": "span-1",
                    }),
                ],
                row_a2a_task_id: Some("agent-task-resumed".to_string()),
                row_phase: StrategyTaskPhase::Running,
                row_error_summary: None,
                row_result_text: None,
            },
        );
    }

    #[sqlx::test(migrations = false)]
    async fn resume_task_rejects_a_second_call_after_the_first_claims_the_row(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_id = insert_test_strategy(&db, "s").await;
        let task_id =
            insert_task_with_phase(&db, strategy_id, StrategyTaskPhase::Failed, "p", None).await;
        let fake = Arc::new(FakeAgentTaskClient::new());
        let agent_client: SharedAgentTaskClient = fake.clone();

        resume_task(&db, &agent_client, task_id)
            .await
            .expect("first resume claims the row");

        let err = resume_task(&db, &agent_client, task_id)
            .await
            .expect_err("second resume must not re-claim an already-running row");
        assert!(
            matches!(&err, ResumeTaskError::NotFailed(id, phase) if *id == task_id && *phase == "running")
        );
        assert_eq!(fake.submitted.lock().await.len(), 1);
    }
}
