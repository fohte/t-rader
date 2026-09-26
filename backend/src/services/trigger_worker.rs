//! cron trigger を schedule どおりに発火させる backend 内の tokio タスク。
//!
//! 1 分間隔で `kind=cron AND enabled=true` の trigger 行を読み、`schedule` (5 フィールド
//! 標準 cron 式、UTC) と `last_fired_at` から発火判定する。発火対象には `fire_trigger`
//! を呼ぶ。発火失敗は次回 tick に持ち越し (`last_fired_at` 更新は `fire_trigger` 内で行われる)。

use std::str::FromStr;
use std::time::Duration;

use chrono::{DateTime, Utc};
use cron::Schedule;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use serde_json::json;

use crate::agent_client::SharedAgentTaskClient;
use crate::entities::trigger;
use crate::services::strategy_tasks::TaskSource;
use crate::services::triggers::{FireTriggerError, fire_trigger};

pub const DEFAULT_INTERVAL: Duration = Duration::from_secs(60);

/// 1 tick で並列に呼び出す `fire_trigger` 数の上限。
const MAX_CONCURRENT_FIRES: usize = 8;

/// POSIX cron の曜日表記 (日曜=0, 月曜=1, ..., 土曜=6, 7 も日曜) を、cron crate が採用する
/// Quartz 表記 (日曜=1, ..., 土曜=7) に変換する。0-7 の範囲外はそのまま返し、後段の
/// `Schedule::from_str` にエラー判定を委ねる。
fn posix_to_quartz_dow(n: u32) -> u32 {
    if n <= 7 { n % 7 + 1 } else { n }
}

/// dow フィールド中の数字トークンだけを POSIX から Quartz に変換する。`1,3,5` (リスト) は
/// 要素ごとに `convert_dow_item` へ委譲する。
fn convert_dow_field(field: &str) -> String {
    field
        .split(',')
        .map(convert_dow_item)
        .collect::<Vec<_>>()
        .join(",")
}

/// dow の 1 要素 (`5` / `1-5` (範囲) / `1-5/2` (ステップ)) を変換する。`MON`-`SUN` の
/// 英字表記は曜日番号ではないためそのまま素通しする。
fn convert_dow_item(item: &str) -> String {
    let (value, step) = match item.split_once('/') {
        Some((v, s)) => (v, Some(s)),
        None => (item, None),
    };
    match value.split_once('-') {
        Some((start, end)) => convert_dow_range(item, start, end, step),
        None => convert_dow_value(value, step),
    }
}

/// 範囲を伴わない単独の値 (`5`, `*` 等) を変換する。
fn convert_dow_value(value: &str, step: Option<&str>) -> String {
    let converted = match value.parse::<u32>() {
        Ok(n) => posix_to_quartz_dow(n).to_string(),
        Err(_) => value.to_string(),
    };
    match step {
        Some(s) => format!("{converted}/{s}"),
        None => converted,
    }
}

/// POSIX の範囲 (`start-end`) を Quartz に変換する。POSIX では 0 と 7 がともに日曜を指すため、
/// 両端を個別に変換すると `5-7` (金-日) が `6-1` のような開始>終了の逆順範囲になり
/// `cron::Schedule::from_str` がエラーにする。これを避けるため、範囲を実際の曜日の集合に
/// 展開してからカンマ区切りリストとして返す。数値でない (英字表記) 範囲・開始>終了の範囲・
/// ステップが数値として解釈できない場合は `original` をそのまま返し、後段の
/// `Schedule::from_str` に判定を委ねる。
fn convert_dow_range(original: &str, start: &str, end: &str, step: Option<&str>) -> String {
    let (Ok(s), Ok(e)) = (start.parse::<u32>(), end.parse::<u32>()) else {
        return original.to_string();
    };
    if s > e {
        return original.to_string();
    }
    let step_n = match step {
        Some(step_str) => match step_str.parse::<usize>() {
            Ok(n) if n > 0 => n,
            _ => return original.to_string(),
        },
        None => 1,
    };
    (s..=e)
        .step_by(step_n)
        .map(posix_to_quartz_dow)
        .map(|n| n.to_string())
        .collect::<Vec<_>>()
        .join(",")
}

/// 5 フィールド標準 cron 式 (`min hour dom month dow`) を 6 フィールド (`sec min hour dom month dow`)
/// に変換して `cron::Schedule` をパースする。秒は 0 固定。フィールド数が 6 以上のときはそのまま流す。
/// dow フィールド (6 番目) は POSIX の曜日表記として解釈し、Quartz 表記に変換してから渡す。
fn parse_schedule(expr: &str) -> Result<Schedule, cron::error::Error> {
    let trimmed = expr.trim();
    let field_count = trimmed.split_whitespace().count();
    let normalized = if field_count == 5 {
        format!("0 {trimmed}")
    } else {
        trimmed.to_string()
    };

    let mut fields: Vec<String> = normalized.split_whitespace().map(str::to_string).collect();
    if let Some(dow) = fields.get_mut(5) {
        *dow = convert_dow_field(dow);
    }
    Schedule::from_str(&fields.join(" "))
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
    db: &impl sea_orm::ConnectionTrait,
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

    let target_count = targets.len();
    let trigger_ids = targets.iter().map(|row| row.trigger_id).collect::<Vec<_>>();
    let results = crate::concurrent::map_concurrent(targets, MAX_CONCURRENT_FIRES, |row| {
        let agent_client = agent_client.clone();
        async move {
            let trigger_id = row.trigger_id;
            match fire_trigger(db, &agent_client, trigger_id, json!({}), TaskSource::Cron).await {
                Ok(_) => {}
                // 取得 → 発火の間に disable された race。
                Err(FireTriggerError::Disabled(_)) => {}
                Err(err) => {
                    tracing::warn!(error = %err, trigger_id = %trigger_id, "cron trigger fire failed");
                }
            }
        }
    })
    .await;
    for (trigger_id, result) in trigger_ids.into_iter().zip(results) {
        if let Err(error) = result {
            tracing::error!(error = %error, trigger_id = %trigger_id, "cron trigger worker task panicked");
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
    #[case::posix_weekday_range("0 9 * * 1-5", "2026-01-04T00:00:00Z", "2026-01-05T09:00:00Z")]
    #[case::posix_sunday_zero("0 9 * * 0", "2026-01-01T00:00:00Z", "2026-01-04T09:00:00Z")]
    #[case::posix_sunday_seven("0 9 * * 7", "2026-01-01T00:00:00Z", "2026-01-04T09:00:00Z")]
    #[case::alpha_weekday_range_unaffected(
        "0 9 * * MON-FRI",
        "2026-01-04T00:00:00Z",
        "2026-01-05T09:00:00Z"
    )]
    #[case::posix_range_crossing_sunday_enters_friday(
        "0 9 * * 5-7",
        "2026-01-08T00:00:00Z",
        "2026-01-09T09:00:00Z"
    )]
    #[case::posix_range_crossing_sunday_skips_weekdays(
        "0 9 * * 5-7",
        "2026-01-11T10:00:00Z",
        "2026-01-16T09:00:00Z"
    )]
    fn parse_schedule_interprets_dow_as_posix(
        #[case] expr: &str,
        #[case] after: &str,
        #[case] expected_next: &str,
    ) {
        let schedule = parse_schedule(expr).unwrap();
        let next = schedule.after(&ts(after)).next().unwrap();
        assert_eq!(next, ts(expected_next));
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
    use crate::services::agent_config;
    use crate::services::strategy_tasks::DEFAULT_PURPOSE;
    use crate::testing::{create_test_db, insert_test_cron_trigger};

    use super::*;

    async fn seed_strategy(db: &impl sea_orm::ConnectionTrait) -> Uuid {
        let id = Uuid::new_v4();
        strategy::ActiveModel {
            id: Set(id),
            name: Set("長期".to_string()),
            description: Set(None),
            sort_order: Set(0),
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
        agent_config::create(&db, DEFAULT_PURPOSE.to_string())
            .await
            .expect("insert test agent_config");
        // 毎分発火する schedule、last_fired_at は十分過去
        let past = Utc.with_ymd_and_hms(2000, 1, 1, 0, 0, 0).unwrap();
        let tid = insert_test_cron_trigger(
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
        let _ = insert_test_cron_trigger(&db, sid, "* * * * *", false, Some(past), "x").await;
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
        let _ = insert_test_cron_trigger(&db, sid, "0 9 * * *", true, Some(just_fired), "x").await;
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
            strategy_id: Set(Some(sid)),
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
