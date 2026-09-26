use sea_orm::ActiveModelTrait;
use sea_orm::ActiveValue::Set;
use sea_orm::sea_query::Expr;
use sea_orm::{
    ColumnTrait, Condition, EntityTrait, QueryFilter, QueryOrder, QuerySelect, QueryTrait,
};
use uuid::Uuid;

use super::{DEADLINE_DURATION, SubmittedTask, phase_str, step_to_wire_json};
use crate::agent_client::{AgentTaskError, SharedAgentTaskClient, SubmitAgentTask};
use crate::entities::sea_orm_active_enums::{StrategyTaskPhase, StrategyTaskStepStatus};
use crate::entities::{strategy_task, strategy_task_step};

#[derive(Debug, thiserror::Error)]
pub enum ResumeTaskError {
    #[error("strategy task {0} not found")]
    NotFound(Uuid),
    #[error("strategy task {0} is not resumable (current phase: {1})")]
    NotResumable(Uuid, &'static str),
    #[error(transparent)]
    Database(#[from] sea_orm::DbErr),
    #[error(transparent)]
    AgentTask(#[from] AgentTaskError),
}

/// failed な戦略タスク、または for_each の部分失敗で failed なステップを抱えたまま
/// completed になった戦略タスクを同じ行のまま再開する。
///
/// 新しい strategy_task 行は作らない (a2a_task_id だけ差し替え、phase を Running に戻す)。
/// 全 strategy_task_step 行 (completed/failed/running 問わず) を `resume_steps` として
/// agent に渡し、どのステップを再利用し、どのステップを再実行するかは agent 側の判断に
/// 委ねる — backend は中身を解釈しない。
pub async fn resume_task(
    db: &impl sea_orm::ConnectionTrait,
    agent_client: &SharedAgentTaskClient,
    task_id: Uuid,
) -> Result<SubmittedTask, ResumeTaskError> {
    resume_task_impl(db, agent_client, task_id, false).await
}

/// execution_lost で failed になった戦略タスクを、watcher が自動で 1 回だけ resume する。
///
/// claim 時点で `auto_resumed_at` を刻むため、投入 (agent への submit) 自体が失敗しても
/// 次回以降は対象から外れる — 呼び出し元 (watcher) は再試行しない。
pub async fn auto_resume_task(
    db: &impl sea_orm::ConnectionTrait,
    agent_client: &SharedAgentTaskClient,
    task_id: Uuid,
) -> Result<SubmittedTask, ResumeTaskError> {
    resume_task_impl(db, agent_client, task_id, true).await
}

async fn resume_task_impl(
    db: &impl sea_orm::ConnectionTrait,
    agent_client: &SharedAgentTaskClient,
    task_id: Uuid,
    mark_auto_resumed: bool,
) -> Result<SubmittedTask, ResumeTaskError> {
    let row = strategy_task::Entity::find_by_id(task_id)
        .one(db)
        .await?
        .ok_or(ResumeTaskError::NotFound(task_id))?;

    // 「再開できる状態である」ことの確認と「Running に倒す」ことを 1 回の条件付き
    // UPDATE で原子化する。事前に phase を読むだけでは、同じ task_id への並行呼び出しが
    // 両方とも再開可能と判断して二重に agent へ投入されうる。
    //
    // Completed は failed ステップを持つ場合だけ再開できる (for_each は 1 件でも成功すれば
    // フェーズ成功として返すため、部分失敗したタスクは Failed でなく Completed で終わる)。
    // Running/Pending は失敗した要素を含んでいても実行中なので対象外。
    let failed_step_task_ids = strategy_task_step::Entity::find()
        .select_only()
        .column(strategy_task_step::Column::TaskId)
        .filter(strategy_task_step::Column::TaskId.eq(task_id))
        .filter(strategy_task_step::Column::Status.eq(StrategyTaskStepStatus::Failed))
        .into_query();
    let resumable = Condition::any()
        .add(strategy_task::Column::Phase.eq(StrategyTaskPhase::Failed))
        .add(
            Condition::all()
                .add(strategy_task::Column::Phase.eq(StrategyTaskPhase::Completed))
                .add(strategy_task::Column::TaskId.in_subquery(failed_step_task_ids)),
        );
    let claim_now = chrono::Utc::now().fixed_offset();
    let mut update = strategy_task::Entity::update_many()
        .col_expr(
            strategy_task::Column::Phase,
            Expr::value(StrategyTaskPhase::Running),
        )
        .col_expr(strategy_task::Column::UpdatedAt, Expr::value(claim_now));
    let mut filter = Condition::all()
        .add(strategy_task::Column::TaskId.eq(task_id))
        .add(resumable);
    if mark_auto_resumed {
        // auto_resumed_at を claim と同じ UPDATE で刻むことで、「1 タスクにつき自動
        // resume は 1 回まで」を後続の再試行と原子的に排他できる (2 回目の呼び出しは
        // ここで claim に失敗する)。
        update = update.col_expr(strategy_task::Column::AutoResumedAt, Expr::value(claim_now));
        filter = filter.add(strategy_task::Column::AutoResumedAt.is_null());
    }
    let claimed = update.filter(filter).exec(db).await?;
    if claimed.rows_affected == 0 {
        return Err(ResumeTaskError::NotResumable(
            task_id,
            phase_str(&row.phase),
        ));
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

    let now = chrono::Utc::now().fixed_offset();
    let deadline_at = now + DEADLINE_DURATION;

    let agent_ref = match agent_client
        .submit(SubmitAgentTask {
            strategy_id: row.strategy_id,
            prompt: row.prompt.clone(),
            purpose: row.purpose.clone(),
            resume_steps: (!resume_steps.is_empty()).then_some(resume_steps),
            deadline_at,
            // 「同じ実行の続き」なので基準時刻は投入時の値のまま渡す (`now` を使わない)。
            as_of: row.as_of,
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

    let running = strategy_task::ActiveModel {
        task_id: Set(task_id),
        a2a_task_id: Set(Some(agent_ref.task_id.clone())),
        error_summary: Set(None),
        result_text: Set(None),
        deadline_at: Set(deadline_at),
        updated_at: Set(chrono::Utc::now().fixed_offset()),
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
/// 同じ id を使い回すことで MCP tool 呼び出しの `x-execution-id` に含まれる step_id 部分が
/// 安定し、ノート書き込み (execution_id で既存ノートを探して更新する) の重複を防ぐ契約のため。
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

    use super::super::TaskSource;
    use super::*;
    use crate::agent_client::{AgentTaskError, FakeAgentTaskClient};
    use crate::entities::sea_orm_active_enums::StrategyTaskStepStatus;
    use crate::testing::insert_test_strategy;
    use sea_orm::ActiveValue::NotSet;

    /// resume 時の `now` と区別できるよう、投入時刻として十分に過去の固定値を使う。
    fn original_as_of() -> chrono::DateTime<chrono::FixedOffset> {
        chrono::DateTime::parse_from_rfc3339("2026-01-01T00:00:00Z").unwrap()
    }

    async fn insert_task_with_phase(
        db: &impl sea_orm::ConnectionTrait,
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
            as_of: Set(Some(original_as_of())),
            auto_resumed_at: NotSet,
            created_at: Set(now),
            updated_at: Set(now),
        }
        .insert(db)
        .await
        .expect("insert test strategy_task");
        task_id
    }

    async fn insert_task_step(
        db: &impl sea_orm::ConnectionTrait,
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

    // database_test は rstest の case 引数を扱わないため、for ループで列挙する。
    #[backend_test_macros::database_test]
    async fn resume_task_rejects_a_task_that_is_neither_failed_nor_completed_with_a_failed_step(
        db: crate::database::DatabaseHandle,
    ) {
        let strategy_id = insert_test_strategy(&db, "s").await;
        let fake = Arc::new(FakeAgentTaskClient::new());
        let agent_client: SharedAgentTaskClient = fake.clone();

        let cases = [
            ("running_without_steps", StrategyTaskPhase::Running, None),
            // 実行中の for_each は失敗済みの要素を持ちうるが、まだ決着していないので対象外。
            (
                "running_with_failed_step",
                StrategyTaskPhase::Running,
                Some(StrategyTaskStepStatus::Failed),
            ),
            (
                "pending_with_failed_step",
                StrategyTaskPhase::Pending,
                Some(StrategyTaskStepStatus::Failed),
            ),
            (
                "completed_without_steps",
                StrategyTaskPhase::Completed,
                None,
            ),
            (
                "completed_with_only_completed_steps",
                StrategyTaskPhase::Completed,
                Some(StrategyTaskStepStatus::Completed),
            ),
        ];
        for (name, phase, step_status) in cases {
            let expected_phase = phase_str(&phase);
            let task_id = insert_task_with_phase(&db, strategy_id, phase, "p", None).await;
            if let Some(status) = step_status {
                insert_task_step(&db, task_id, Uuid::new_v4(), "investigate", status, 1).await;
            }

            let err = resume_task(&db, &agent_client, task_id)
                .await
                .expect_err(name);
            assert!(
                matches!(&err, ResumeTaskError::NotResumable(id, phase) if *id == task_id && *phase == expected_phase),
                "{name}: {err:?}",
            );
        }
        assert!(fake.submitted.lock().await.is_empty());
    }

    #[backend_test_macros::database_test]
    async fn resume_task_not_found(db: crate::database::DatabaseHandle) {
        let agent_client: SharedAgentTaskClient = Arc::new(FakeAgentTaskClient::new());
        let task_id = Uuid::new_v4();

        let err = resume_task(&db, &agent_client, task_id)
            .await
            .expect_err("missing task should fail");
        assert!(matches!(err, ResumeTaskError::NotFound(id) if id == task_id));
    }

    // database_test は rstest の case 引数を扱わないため、for ループで列挙する。
    // Completed は for_each の部分失敗で failed なステップを抱えたまま終わったタスク。
    #[backend_test_macros::database_test]
    async fn resume_task_resubmits_all_steps_and_updates_the_row_in_place(
        db: crate::database::DatabaseHandle,
    ) {
        let strategy_id = insert_test_strategy(&db, "s").await;
        for start_phase in [StrategyTaskPhase::Failed, StrategyTaskPhase::Completed] {
            let start_label = phase_str(&start_phase);
            let task_id = insert_task_with_phase(
                &db,
                strategy_id,
                start_phase,
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
            // a2a_task_id は UNIQUE のため周回ごとに変える。
            let a2a_task_id = format!("agent-task-resumed-{start_label}");
            fake.set_next_task_id(&a2a_task_id).await;
            let agent_client: SharedAgentTaskClient = fake.clone();
            let started_at = chrono::DateTime::parse_from_rfc3339("2026-01-01T00:00:00Z")
                .unwrap()
                .to_rfc3339();

            let submitted = resume_task(&db, &agent_client, task_id)
                .await
                .expect("resume ok");

            let (submitted_resume_steps, submitted_as_of) = {
                let submitted = fake.submitted.lock().await;
                let req = submitted.first().expect("submit called once");
                (
                    req.resume_steps.clone().expect("resume_steps present"),
                    req.as_of,
                )
            };
            let row = strategy_task::Entity::find_by_id(task_id)
                .one(&db)
                .await
                .unwrap()
                .expect("row still exists");

            #[derive(Debug, PartialEq)]
            struct ResumeOutcome {
                submitted_a2a_task_id: String,
                resume_steps: Vec<serde_json::Value>,
                submitted_as_of: Option<chrono::DateTime<chrono::FixedOffset>>,
                row_a2a_task_id: Option<String>,
                row_phase: StrategyTaskPhase,
                row_error_summary: Option<String>,
                row_result_text: Option<String>,
                row_as_of: Option<chrono::DateTime<chrono::FixedOffset>>,
            }

            assert_eq!(
                ResumeOutcome {
                    submitted_a2a_task_id: submitted.a2a_task_id,
                    resume_steps: submitted_resume_steps,
                    submitted_as_of,
                    row_a2a_task_id: row.a2a_task_id,
                    row_phase: row.phase,
                    row_error_summary: row.error_summary,
                    row_result_text: row.result_text,
                    row_as_of: row.as_of,
                },
                ResumeOutcome {
                    submitted_a2a_task_id: a2a_task_id.clone(),
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
                    submitted_as_of: Some(original_as_of()),
                    row_a2a_task_id: Some(a2a_task_id.clone()),
                    row_phase: StrategyTaskPhase::Running,
                    row_error_summary: None,
                    row_result_text: None,
                    row_as_of: Some(original_as_of()),
                },
                "start phase: {start_label}",
            );
        }
    }

    #[backend_test_macros::database_test]
    async fn resume_task_rejects_a_second_call_after_the_first_claims_the_row(
        db: crate::database::DatabaseHandle,
    ) {
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
            matches!(&err, ResumeTaskError::NotResumable(id, phase) if *id == task_id && *phase == "running")
        );
        assert_eq!(fake.submitted.lock().await.len(), 1);
    }

    #[backend_test_macros::database_test]
    async fn auto_resume_task_resubmits_and_marks_auto_resumed(
        db: crate::database::DatabaseHandle,
    ) {
        let strategy_id = insert_test_strategy(&db, "s").await;
        let task_id =
            insert_task_with_phase(&db, strategy_id, StrategyTaskPhase::Failed, "p", None).await;
        let fake = Arc::new(FakeAgentTaskClient::new());
        fake.set_next_task_id("agent-task-auto-resumed").await;
        let agent_client: SharedAgentTaskClient = fake.clone();

        let submitted = auto_resume_task(&db, &agent_client, task_id)
            .await
            .expect("auto resume ok");
        let row = strategy_task::Entity::find_by_id(task_id)
            .one(&db)
            .await
            .unwrap()
            .expect("row still exists");

        #[derive(Debug, PartialEq)]
        struct AutoResumeOutcome {
            submitted_a2a_task_id: String,
            row_a2a_task_id: Option<String>,
            row_phase: StrategyTaskPhase,
            row_auto_resumed_at_is_set: bool,
        }

        assert_eq!(
            AutoResumeOutcome {
                submitted_a2a_task_id: submitted.a2a_task_id,
                row_a2a_task_id: row.a2a_task_id,
                row_phase: row.phase,
                row_auto_resumed_at_is_set: row.auto_resumed_at.is_some(),
            },
            AutoResumeOutcome {
                submitted_a2a_task_id: "agent-task-auto-resumed".to_string(),
                row_a2a_task_id: Some("agent-task-auto-resumed".to_string()),
                row_phase: StrategyTaskPhase::Running,
                row_auto_resumed_at_is_set: true,
            },
        );
        assert_eq!(fake.submitted.lock().await.len(), 1);
    }

    #[backend_test_macros::database_test]
    async fn auto_resume_task_rejects_a_second_call_once_already_auto_resumed(
        db: crate::database::DatabaseHandle,
    ) {
        let strategy_id = insert_test_strategy(&db, "s").await;
        let task_id =
            insert_task_with_phase(&db, strategy_id, StrategyTaskPhase::Failed, "p", None).await;
        let fake = Arc::new(FakeAgentTaskClient::new());
        let agent_client: SharedAgentTaskClient = fake.clone();

        auto_resume_task(&db, &agent_client, task_id)
            .await
            .expect("first auto resume claims the row");

        // 手動 resume で失敗を再現する代わりに、直接 phase を Failed に戻す。
        // auto_resumed_at は前回の claim で既に刻まれているため、この UPDATE では触らない。
        strategy_task::ActiveModel {
            task_id: Set(task_id),
            phase: Set(StrategyTaskPhase::Failed),
            updated_at: Set(chrono::Utc::now().fixed_offset()),
            ..Default::default()
        }
        .update(&db)
        .await
        .expect("force phase back to failed");

        let err = auto_resume_task(&db, &agent_client, task_id)
            .await
            .expect_err("second auto resume must not re-claim an already auto-resumed row");
        assert!(
            matches!(&err, ResumeTaskError::NotResumable(id, phase) if *id == task_id && *phase == "failed")
        );
        assert_eq!(fake.submitted.lock().await.len(), 1);
    }

    #[backend_test_macros::database_test]
    async fn auto_resume_task_marks_auto_resumed_at_even_when_submission_fails(
        db: crate::database::DatabaseHandle,
    ) {
        let strategy_id = insert_test_strategy(&db, "s").await;
        let task_id =
            insert_task_with_phase(&db, strategy_id, StrategyTaskPhase::Failed, "p", None).await;
        let fake = Arc::new(FakeAgentTaskClient::new());
        fake.set_submit_error(AgentTaskError::NotConfigured).await;
        let agent_client: SharedAgentTaskClient = fake.clone();

        let err = auto_resume_task(&db, &agent_client, task_id)
            .await
            .expect_err("submission failure must surface as an error");
        assert!(matches!(err, ResumeTaskError::AgentTask(_)));

        let row = strategy_task::Entity::find_by_id(task_id)
            .one(&db)
            .await
            .unwrap()
            .expect("row still exists");
        assert!(row.auto_resumed_at.is_some());
    }

    #[backend_test_macros::database_test]
    async fn resume_task_does_not_touch_auto_resumed_at(db: crate::database::DatabaseHandle) {
        let strategy_id = insert_test_strategy(&db, "s").await;
        let task_id =
            insert_task_with_phase(&db, strategy_id, StrategyTaskPhase::Failed, "p", None).await;
        let agent_client: SharedAgentTaskClient = Arc::new(FakeAgentTaskClient::new());

        resume_task(&db, &agent_client, task_id)
            .await
            .expect("manual resume ok");

        let row = strategy_task::Entity::find_by_id(task_id)
            .one(&db)
            .await
            .unwrap()
            .expect("row still exists");
        assert!(row.auto_resumed_at.is_none());
    }
}
