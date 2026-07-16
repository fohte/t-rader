//! cron trigger を schedule どおりに発火させる backend 内の tokio タスク。
//!
//! 1 分間隔で `kind=cron AND enabled=true` の trigger 行を読み、`schedule` (5 フィールド
//! 標準 cron 式、UTC) と `last_fired_at` から発火判定する。発火対象には `fire_trigger`
//! を呼ぶ。発火失敗は次回 tick に持ち越し (`last_fired_at` 更新は `fire_trigger` 内で行われる)。

use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Utc};
use cron::Schedule;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use serde_json::json;
use tokio::sync::Semaphore;

use crate::agent_client::SharedAgentTaskClient;
use crate::entities::trigger;
use crate::services::strategy_tasks::TaskSource;
use crate::services::triggers::{FireTriggerError, fire_trigger};

pub const DEFAULT_INTERVAL: Duration = Duration::from_secs(60);

/// 1 tick で並列に呼び出す `fire_trigger` 数の上限。
const MAX_CONCURRENT_FIRES: usize = 8;

/// 5 フィールド標準 cron 式 (`min hour dom month dow`) を 6 フィールド (`sec min hour dom month dow`)
/// に変換して `cron::Schedule` をパースする。秒は 0 固定。フィールド数が 6 以上のときはそのまま流す。
fn parse_schedule(expr: &str) -> Result<Schedule, cron::error::Error> {
    let trimmed = expr.trim();
    let field_count = trimmed.split_whitespace().count();
    let normalized = if field_count == 5 {
        format!("0 {trimmed}")
    } else {
        trimmed.to_string()
    };
    Schedule::from_str(&normalized)
}

/// `schedule` と `last_fired_at` から、`now` 時点で発火すべきか判定する。
///
/// `last_fired_at` 直後の次回発火時刻 (`schedule.after(last_fired_at).next()`) が `now` 以下なら
/// 発火対象。`last_fired_at` が NULL の trigger は「現 tick の interval 直前」を起点に評価する。
/// `interval` には worker の tick 間隔を渡す。
fn should_fire(
    schedule: &Schedule,
    last_fired_at: Option<DateTime<Utc>>,
    now: DateTime<Utc>,
    interval: Duration,
) -> bool {
    let after = last_fired_at.unwrap_or_else(|| {
        now - chrono::Duration::from_std(interval).unwrap_or(chrono::Duration::zero())
    });
    schedule
        .after(&after)
        .next()
        .is_some_and(|next| next <= now)
}

/// 1 tick ぶんの発火判定 + 発火実行。
///
/// 戻り値は発火を試みた件数 (成功 / 失敗を問わない)。`interval` には worker の tick 間隔を渡す。
pub async fn run_once(
    db: &DatabaseConnection,
    agent_client: &SharedAgentTaskClient,
    interval: Duration,
) -> usize {
    let rows = match trigger::Entity::find()
        .filter(trigger::Column::Kind.eq("cron"))
        .filter(trigger::Column::Enabled.eq(true))
        .all(db)
        .await
    {
        Ok(rows) => rows,
        Err(err) => {
            tracing::warn!(error = %err, "failed to list cron triggers");
            return 0;
        }
    };

    // schedule と last_fired_at を見て、発火対象だけを抽出する。schedule パース失敗は
    // 個別にスキップ (log) し、他の trigger を巻き込まない。
    let now = Utc::now();
    let targets: Vec<trigger::Model> = rows
        .into_iter()
        .filter(|row| {
            let Some(expr) = row.schedule.as_deref() else {
                tracing::warn!(trigger_id = %row.trigger_id, "cron trigger has no schedule; skip");
                return false;
            };
            match parse_schedule(expr) {
                Ok(schedule) => {
                    let last = row.last_fired_at.map(|dt| dt.with_timezone(&Utc));
                    should_fire(&schedule, last, now, interval)
                }
                Err(err) => {
                    tracing::warn!(
                        trigger_id = %row.trigger_id,
                        schedule = expr,
                        error = %err,
                        "failed to parse cron schedule; skip",
                    );
                    false
                }
            }
        })
        .collect();

    let semaphore = Arc::new(Semaphore::new(MAX_CONCURRENT_FIRES));
    let target_count = targets.len();
    let mut handles = Vec::with_capacity(target_count);
    for row in targets {
        let agent_client = agent_client.clone();
        let db = db.clone();
        let sem = semaphore.clone();
        let trigger_id = row.trigger_id;
        handles.push(tokio::spawn(async move {
            let Ok(_permit) = sem.acquire_owned().await else {
                return;
            };
            match fire_trigger(&db, &agent_client, trigger_id, json!({}), TaskSource::Cron).await {
                Ok(_) => {}
                // 取得 → 発火の間に disable された race。
                Err(FireTriggerError::Disabled(_)) => {}
                Err(err) => {
                    tracing::warn!(error = %err, trigger_id = %trigger_id, "cron trigger fire failed");
                }
            }
        }));
    }
    for handle in handles {
        if let Err(err) = handle.await {
            // JoinError は子タスク panic / cancel。watcher と同じく log するに留め、
            // 他 trigger の処理を継続する。
            tracing::error!(error = %err, "cron trigger worker task panicked");
        }
    }
    target_count
}

/// 定期 polling のバックグラウンドタスクを起動する。
pub fn spawn(
    db: DatabaseConnection,
    agent_client: SharedAgentTaskClient,
    interval: Duration,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(interval);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        // 起動直後の即時実行は避ける (initial delay)。
        ticker.tick().await;
        loop {
            ticker.tick().await;
            let attempts = run_once(&db, &agent_client, interval).await;
            if attempts > 0 {
                tracing::info!(attempts, "cron triggers evaluated");
            }
        }
    })
}

#[cfg(test)]
mod parse_tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case::five_field("0 9 * * 1-5")]
    #[case::with_leading_whitespace("  0 9 * * 1-5  ")]
    #[case::six_field("0 0 9 * * 1-5")]
    fn parse_schedule_accepts(#[case] expr: &str) {
        assert!(parse_schedule(expr).is_ok());
    }

    fn ts(s: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(s).unwrap().with_timezone(&Utc)
    }

    #[rstest]
    // last_fired_at=None で「現在時刻ちょうど」が schedule に乗っていれば発火する。
    #[case::first_time_within_window(
        "* * * * *",
        None,
        "2026-01-01T09:00:00Z",
        DEFAULT_INTERVAL,
        true
    )]
    // 9:00 のみ発火。最後の発火が 9:00 なら次の 10:00 まで発火しない。
    #[case::last_fired_after_latest_slot(
        "0 9 * * *",
        Some("2026-01-01T09:00:00Z"),
        "2026-01-01T09:30:00Z",
        DEFAULT_INTERVAL,
        false
    )]
    // 9:00 と 10:00 発火。9:30 時点で last=9:00 なら次の発火 10:00 はまだ。
    #[case::schedule_slot_not_passed_yet(
        "0 9,10 * * *",
        Some("2026-01-01T09:00:00Z"),
        "2026-01-01T09:30:00Z",
        DEFAULT_INTERVAL,
        false
    )]
    // 10:30 時点で last=9:00 なら 10:00 を過ぎているので発火する。
    #[case::schedule_slot_passed(
        "0 9,10 * * *",
        Some("2026-01-01T09:00:00Z"),
        "2026-01-01T10:30:00Z",
        DEFAULT_INTERVAL,
        true
    )]
    // now=9:00:30、default interval (60s) なら 8:59:30 起点で 9:00 を拾い発火する。
    #[case::first_time_window_respects_default_interval(
        "0 9 * * *",
        None,
        "2026-01-01T09:00:30Z",
        DEFAULT_INTERVAL,
        true
    )]
    // 同 now で interval=10s なら 9:00:20 起点で 9:00 を拾えず発火しない。
    #[case::first_time_window_respects_short_interval(
        "0 9 * * *",
        None,
        "2026-01-01T09:00:30Z",
        Duration::from_secs(10),
        false
    )]
    fn should_fire_cases(
        #[case] expr: &str,
        #[case] last_fired_at: Option<&str>,
        #[case] now: &str,
        #[case] interval: Duration,
        #[case] expected: bool,
    ) {
        let schedule = parse_schedule(expr).unwrap();
        let last = last_fired_at.map(ts);
        assert_eq!(should_fire(&schedule, last, ts(now), interval), expected);
    }
}

#[cfg(test)]
mod run_once_tests {
    use std::sync::Arc;

    use chrono::TimeZone;
    use sea_orm::ActiveModelTrait;
    use sea_orm::ActiveValue::{NotSet, Set};
    use sea_orm::{EntityTrait, QueryOrder};
    use sqlx::PgPool;
    use uuid::Uuid;

    use crate::agent_client::{FakeAgentTaskClient, SharedAgentTaskClient};
    use crate::entities::sea_orm_active_enums::StrategyTaskPhase;
    use crate::entities::{strategy, strategy_task};
    use crate::testing::create_test_db;

    use super::*;

    async fn seed_strategy(db: &DatabaseConnection) -> Uuid {
        let id = Uuid::new_v4();
        strategy::ActiveModel {
            id: Set(id),
            name: Set("長期".to_string()),
            description: Set(None),
            sort_order: Set(0),
            agents_md: NotSet,
            skills: NotSet,
            agent_status: NotSet,
            agent_error: NotSet,
            created_at: NotSet,
            updated_at: NotSet,
        }
        .insert(db)
        .await
        .unwrap();
        id
    }

    async fn insert_cron_trigger(
        db: &DatabaseConnection,
        strategy_id: Uuid,
        schedule: &str,
        enabled: bool,
        last_fired_at: Option<DateTime<Utc>>,
        prompt: &str,
    ) -> Uuid {
        let id = Uuid::new_v4();
        trigger::ActiveModel {
            trigger_id: Set(id),
            strategy_id: Set(strategy_id),
            kind: Set("cron".to_string()),
            schedule: Set(Some(schedule.to_string())),
            hook_slug: Set(None),
            event_match: Set(None),
            prompt_template: Set(prompt.to_string()),
            enabled: Set(enabled),
            last_fired_at: Set(last_fired_at.map(|dt| dt.fixed_offset())),
            created_at: NotSet,
            updated_at: NotSet,
        }
        .insert(db)
        .await
        .unwrap();
        id
    }

    /// strategy_task の動的フィールドを落とした比較用ビュー。
    #[derive(Debug, PartialEq, Eq)]
    struct TaskShape {
        strategy_id: Uuid,
        source: String,
        prompt: String,
        phase: StrategyTaskPhase,
    }

    impl TaskShape {
        fn from(row: &strategy_task::Model) -> Self {
            Self {
                strategy_id: row.strategy_id,
                source: row.source.clone(),
                prompt: row.prompt.clone(),
                phase: row.phase.clone(),
            }
        }
    }

    #[sqlx::test(migrations = false)]
    async fn fires_due_cron_and_writes_strategy_task(pool: PgPool) {
        let db = create_test_db(pool).await;
        let sid = seed_strategy(&db).await;
        // 毎分発火する schedule、last_fired_at は十分過去
        let past = Utc.with_ymd_and_hms(2000, 1, 1, 0, 0, 0).unwrap();
        let tid = insert_cron_trigger(
            &db,
            sid,
            "* * * * *",
            true,
            Some(past),
            "{{strategy.name}} morning",
        )
        .await;
        let kube: SharedAgentTaskClient = Arc::new(FakeAgentTaskClient::new());

        let attempts = run_once(&db, &kube, DEFAULT_INTERVAL).await;
        assert_eq!(attempts, 1);

        let tasks = strategy_task::Entity::find()
            .order_by_asc(strategy_task::Column::CreatedAt)
            .all(&db)
            .await
            .unwrap();
        let fired = trigger::Entity::find_by_id(tid)
            .one(&db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            (
                tasks.iter().map(TaskShape::from).collect::<Vec<_>>(),
                fired.last_fired_at.is_some(),
            ),
            (
                vec![TaskShape {
                    strategy_id: sid,
                    source: "cron".to_string(),
                    prompt: "長期 morning".to_string(),
                    phase: StrategyTaskPhase::Running,
                }],
                true,
            ),
        );
    }

    #[sqlx::test(migrations = false)]
    async fn skips_disabled_cron(pool: PgPool) {
        let db = create_test_db(pool).await;
        let sid = seed_strategy(&db).await;
        let past = Utc.with_ymd_and_hms(2000, 1, 1, 0, 0, 0).unwrap();
        let _ = insert_cron_trigger(&db, sid, "* * * * *", false, Some(past), "x").await;
        let kube: SharedAgentTaskClient = Arc::new(FakeAgentTaskClient::new());

        let attempts = run_once(&db, &kube, DEFAULT_INTERVAL).await;
        assert_eq!(attempts, 0);

        let tasks = strategy_task::Entity::find().all(&db).await.unwrap();
        assert!(tasks.is_empty());
    }

    #[sqlx::test(migrations = false)]
    async fn skips_when_no_slot_after_last_fire(pool: PgPool) {
        // 9:00 だけ発火する schedule で「直前に発火済み + 次回 9:00 はまだ先」のケース。
        // last_fired_at を「現時刻直前」に置いて、現 tick では発火対象にならないことを確認する。
        let db = create_test_db(pool).await;
        let sid = seed_strategy(&db).await;
        let just_fired = Utc::now() - chrono::Duration::seconds(1);
        let _ = insert_cron_trigger(&db, sid, "0 9 * * *", true, Some(just_fired), "x").await;
        let kube: SharedAgentTaskClient = Arc::new(FakeAgentTaskClient::new());

        let attempts = run_once(&db, &kube, DEFAULT_INTERVAL).await;
        assert_eq!(attempts, 0);
        let tasks = strategy_task::Entity::find().all(&db).await.unwrap();
        assert!(tasks.is_empty());
    }

    #[sqlx::test(migrations = false)]
    async fn ignores_hook_kind(pool: PgPool) {
        // hook 種別の trigger は cron worker の対象外。
        let db = create_test_db(pool).await;
        let sid = seed_strategy(&db).await;
        let id = Uuid::new_v4();
        trigger::ActiveModel {
            trigger_id: Set(id),
            strategy_id: Set(sid),
            kind: Set("hook".to_string()),
            schedule: Set(None),
            hook_slug: Set(Some("h".to_string())),
            event_match: Set(None),
            prompt_template: Set("x".to_string()),
            enabled: Set(true),
            last_fired_at: NotSet,
            created_at: NotSet,
            updated_at: NotSet,
        }
        .insert(&db)
        .await
        .unwrap();
        let kube: SharedAgentTaskClient = Arc::new(FakeAgentTaskClient::new());

        let attempts = run_once(&db, &kube, DEFAULT_INTERVAL).await;
        assert_eq!(attempts, 0);
    }
}
