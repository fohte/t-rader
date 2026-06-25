//! trigger 発火と payload マッチ判定の共通 service。
//!
//! cron worker と hook endpoint の双方から呼ばれる想定。本ファイルでは fire の
//! オーケストレーション、`prompt_template` 展開、`event_match` 評価を提供する。
//! cron スケジュール解釈と hook endpoint 実装は別レイヤ。

use chrono::Utc;
use sea_orm::ActiveModelTrait;
use sea_orm::ActiveValue::Set;
use sea_orm::{DatabaseConnection, EntityTrait, IntoActiveModel};
use serde_json::Value;
use uuid::Uuid;

use crate::entities::{strategy, trigger};
use crate::kubeopencode::SharedKubeopencodeClient;
use crate::services::strategy_tasks::{
    SubmitStrategyTaskError, SubmitStrategyTaskOutcome, submit_strategy_task,
};

/// `strategy_task.source` に書き込む値の集合。fire 元を識別する。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TriggerSource {
    Cron,
    Hook,
}

impl TriggerSource {
    pub const fn as_str(self) -> &'static str {
        match self {
            TriggerSource::Cron => "cron",
            TriggerSource::Hook => "hook",
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum FireTriggerError {
    #[error("trigger {0} not found")]
    TriggerNotFound(Uuid),
    #[error("trigger {0} is disabled")]
    Disabled(Uuid),
    #[error("submit strategy task failed: {0}")]
    Submit(#[from] SubmitStrategyTaskError),
    #[error("database error: {0}")]
    Database(#[from] sea_orm::DbErr),
}

/// trigger を発火する。
///
/// 1. trigger を取得
/// 2. 戦略名を取得して標準 context (`now`, `strategy.name`) を構築
/// 3. `prompt_template` を payload + context で展開
/// 4. `submit_strategy_task` で `strategy_task` 行を作成
/// 5. submit 成功後に `last_fired_at` を更新
///
/// submit と `last_fired_at` 更新は別トランザクションのため、submit 成功直後に DB が落ちると
/// タスクが投入済みなのに `last_fired_at` が古いまま残る。次回 worker 走査で再発火する可能性
/// があるので、`kubeopencode_task_name` の衝突回避はランダム部分に任せ、呼び出し側は重複発火
/// を許容する前提で組むこと。
pub async fn fire_trigger(
    db: &DatabaseConnection,
    kube: &SharedKubeopencodeClient,
    trigger_id: Uuid,
    payload: Value,
    source: TriggerSource,
) -> Result<SubmitStrategyTaskOutcome, FireTriggerError> {
    let trigger_row = trigger::Entity::find_by_id(trigger_id)
        .one(db)
        .await?
        .ok_or(FireTriggerError::TriggerNotFound(trigger_id))?;
    if !trigger_row.enabled {
        return Err(FireTriggerError::Disabled(trigger_id));
    }

    let strategy_row = strategy::Entity::find_by_id(trigger_row.strategy_id)
        .one(db)
        .await?
        .ok_or(FireTriggerError::Submit(
            SubmitStrategyTaskError::StrategyNotFound(trigger_row.strategy_id),
        ))?;

    // prompt 内 `{{now}}` と DB の `last_fired_at` を同一時点に揃えるため now を 1 度だけ取る
    let now = Utc::now();
    let context = build_standard_context(&strategy_row, now);
    let prompt = expand_template(&trigger_row.prompt_template, &payload, &context);

    let outcome =
        submit_strategy_task(db, kube, trigger_row.strategy_id, prompt, source.as_str()).await?;

    let now_fixed = now.fixed_offset();
    let mut active = trigger_row.into_active_model();
    active.last_fired_at = Set(Some(now_fixed));
    active.updated_at = Set(now_fixed);
    active.update(db).await?;

    Ok(outcome)
}

/// 標準 context (`now`, `strategy.name`) を構築する。
fn build_standard_context(strategy_row: &strategy::Model, now: chrono::DateTime<Utc>) -> Value {
    serde_json::json!({
        "now": now.to_rfc3339(),
        "strategy": {
            "id": strategy_row.id.to_string(),
            "name": strategy_row.name,
        },
    })
}

/// `{{path.to.field}}` 形式の placeholder を展開する。
///
/// - `{{payload.X.Y}}` は payload root 配下の dot-path
/// - `{{now}}` / `{{strategy.name}}` 等は context root 配下の dot-path
/// - 解決できないキー / 配列等の非 object 経路は空文字で置換
/// - 値が string なら裸の文字列、それ以外なら JSON 文字列化した結果を埋め込む
pub fn expand_template(template: &str, payload: &Value, context: &Value) -> String {
    let mut out = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(open) = rest.find("{{") {
        out.push_str(&rest[..open]);
        let after_open = &rest[open + 2..];
        match after_open.find("}}") {
            Some(close) => {
                let inner = after_open[..close].trim();
                out.push_str(&resolve_path(inner, payload, context));
                rest = &after_open[close + 2..];
            }
            None => {
                // 閉じ括弧が無いので残りはそのまま出力して終わる
                out.push_str("{{");
                rest = after_open;
                break;
            }
        }
    }
    out.push_str(rest);
    out
}

/// dot-path を解決して文字列化する。
fn resolve_path(path: &str, payload: &Value, context: &Value) -> String {
    if path.is_empty() {
        return String::new();
    }
    let mut parts = path.split('.');
    let head = parts.next().unwrap_or("");
    let rest: Vec<&str> = parts.collect();
    let root = match head {
        "payload" => payload,
        _ => context,
    };
    let target = if head == "payload" {
        walk(root, &rest)
    } else {
        // context root 直下の field 名を head として扱うため、head も含めて辿る
        let mut full = vec![head];
        full.extend(rest);
        walk(root, &full)
    };
    match target {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Null) | None => String::new(),
        Some(v) => v.to_string(),
    }
}

fn walk<'a>(root: &'a Value, parts: &[&str]) -> Option<&'a Value> {
    let mut current = root;
    for p in parts {
        current = current.as_object()?.get(*p)?;
    }
    Some(current)
}

/// `event_match` を payload に対して評価する。
///
/// 形式: `{"path.to.field": {"eq": <value>}}` または `{"path.to.field": {"exists": true|false}}`
/// の AND。条件が空 / null なら常に真。配列内アクセスは未サポート (object のみ)。
pub fn evaluate_event_match(event_match: Option<&Value>, payload: &Value) -> bool {
    let Some(spec) = event_match else { return true };
    if spec.is_null() {
        return true;
    }
    let Some(map) = spec.as_object() else {
        return false;
    };
    for (path, cond) in map {
        let parts: Vec<&str> = path.split('.').collect();
        let actual = walk(payload, &parts);
        let Some(cond_map) = cond.as_object() else {
            return false;
        };
        if let Some(expected) = cond_map.get("eq") {
            match actual {
                Some(v) if v == expected => {}
                _ => return false,
            }
        }
        if let Some(exists) = cond_map.get("exists").and_then(Value::as_bool) {
            let has = matches!(actual, Some(v) if !v.is_null());
            if has != exists {
                return false;
            }
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use indoc::indoc;
    use rstest::rstest;
    use serde_json::json;

    use super::*;

    fn ctx() -> Value {
        json!({
            "now": "2026-01-01T00:00:00Z",
            "strategy": { "id": "abc", "name": "長期" },
        })
    }

    #[rstest]
    #[case::no_placeholders("hello world", json!({}), "hello world")]
    #[case::single_payload("symbol={{payload.symbol}}", json!({"symbol": "7203"}), "symbol=7203")]
    #[case::nested_payload(
        "p={{payload.a.b.c}}",
        json!({"a": {"b": {"c": "x"}}}),
        "p=x"
    )]
    #[case::context_strategy_name(
        "strategy={{strategy.name}}",
        json!({}),
        "strategy=長期"
    )]
    #[case::context_now("at={{now}}", json!({}), "at=2026-01-01T00:00:00Z")]
    #[case::missing_path_is_blank("v=[{{payload.missing}}]", json!({}), "v=[]")]
    #[case::number_to_json_string(
        "n={{payload.n}}",
        json!({"n": 42}),
        "n=42"
    )]
    #[case::whitespace_in_placeholder(
        "v={{ payload.x }}",
        json!({"x": "y"}),
        "v=y"
    )]
    #[case::lone_open_brace_kept("{ not a placeholder", json!({}), "{ not a placeholder")]
    fn expand_template_cases(
        #[case] template: &str,
        #[case] payload: Value,
        #[case] expected: &str,
    ) {
        assert_eq!(expand_template(template, &payload, &ctx()), expected);
    }

    #[test]
    fn expand_template_multiline_with_indoc() {
        let template = indoc! {"
            銘柄: {{payload.symbol}}
            戦略: {{strategy.name}}
        "};
        let payload = json!({"symbol": "7203"});
        let expected = indoc! {"
            銘柄: 7203
            戦略: 長期
        "};
        assert_eq!(expand_template(template, &payload, &ctx()), expected);
    }

    #[rstest]
    #[case::no_spec(None, json!({"a": 1}), true)]
    #[case::null_spec(Some(json!(null)), json!({"a": 1}), true)]
    #[case::eq_match(
        Some(json!({"event": {"eq": "fired"}})),
        json!({"event": "fired"}),
        true
    )]
    #[case::eq_miss(
        Some(json!({"event": {"eq": "fired"}})),
        json!({"event": "other"}),
        false
    )]
    #[case::exists_true(
        Some(json!({"symbol": {"exists": true}})),
        json!({"symbol": "7203"}),
        true
    )]
    #[case::exists_false_match(
        Some(json!({"symbol": {"exists": false}})),
        json!({}),
        true
    )]
    #[case::nested_path(
        Some(json!({"a.b": {"eq": "x"}})),
        json!({"a": {"b": "x"}}),
        true
    )]
    #[case::and_all_match(
        Some(json!({
            "event": {"eq": "fired"},
            "symbol": {"exists": true},
        })),
        json!({"event": "fired", "symbol": "7203"}),
        true
    )]
    #[case::and_one_fails(
        Some(json!({
            "event": {"eq": "fired"},
            "symbol": {"exists": true},
        })),
        json!({"event": "fired"}),
        false
    )]
    fn event_match_cases(
        #[case] spec: Option<Value>,
        #[case] payload: Value,
        #[case] expected: bool,
    ) {
        assert_eq!(evaluate_event_match(spec.as_ref(), &payload), expected);
    }
}

#[cfg(test)]
mod fire_tests {
    use std::sync::Arc;

    use sea_orm::ActiveValue::{NotSet, Set};
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
    use serde_json::json;
    use sqlx::PgPool;

    use crate::entities::sea_orm_active_enums::StrategyAgentStatus;
    use crate::entities::{strategy, strategy_task, trigger};
    use crate::kubeopencode::{FakeKubeopencodeClient, SharedKubeopencodeClient};
    use crate::testing::create_test_db;

    use super::*;

    async fn seed_strategy(db: &sea_orm::DatabaseConnection, name: &str) -> Uuid {
        let id = Uuid::new_v4();
        strategy::ActiveModel {
            id: Set(id),
            name: Set(name.to_string()),
            description: Set(None),
            sort_order: Set(0),
            agents_md: NotSet,
            skills: NotSet,
            // submit_strategy_task は Ready のみ受け付けるため Ready で投入
            agent_status: Set(StrategyAgentStatus::Ready),
            agent_error: NotSet,
            created_at: NotSet,
            updated_at: NotSet,
        }
        .insert(db)
        .await
        .unwrap();
        id
    }

    async fn seed_hook_trigger(
        db: &sea_orm::DatabaseConnection,
        strategy_id: Uuid,
        slug: &str,
        prompt_template: &str,
    ) -> Uuid {
        let id = Uuid::new_v4();
        trigger::ActiveModel {
            trigger_id: Set(id),
            strategy_id: Set(strategy_id),
            kind: Set("hook".to_string()),
            schedule: Set(None),
            hook_slug: Set(Some(slug.to_string())),
            event_match: Set(None),
            prompt_template: Set(prompt_template.to_string()),
            enabled: Set(true),
            last_fired_at: NotSet,
            created_at: NotSet,
            updated_at: NotSet,
        }
        .insert(db)
        .await
        .unwrap();
        id
    }

    /// strategy_task の値域から動的フィールドを落とした比較用ビュー。
    #[derive(Debug, PartialEq, Eq)]
    struct TaskShape {
        strategy_id: Uuid,
        source: String,
        prompt: String,
        phase: crate::entities::sea_orm_active_enums::StrategyTaskPhase,
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

    /// trigger 行の `last_fired_at` を「発火済みか否か」に正規化した比較用ビュー。
    #[derive(Debug, PartialEq, Eq)]
    struct TriggerFireShape {
        trigger_id: Uuid,
        enabled: bool,
        kind: String,
        last_fired: bool,
    }

    impl TriggerFireShape {
        fn from(row: &trigger::Model) -> Self {
            Self {
                trigger_id: row.trigger_id,
                enabled: row.enabled,
                kind: row.kind.clone(),
                last_fired: row.last_fired_at.is_some(),
            }
        }
    }

    #[sqlx::test(migrations = false)]
    async fn fire_creates_strategy_task_with_expected_source(pool: PgPool) {
        let db = create_test_db(pool).await;
        let sid = seed_strategy(&db, "長期").await;
        let tid = seed_hook_trigger(&db, sid, "tv", "alert {{payload.symbol}}").await;
        let kube: SharedKubeopencodeClient = Arc::new(FakeKubeopencodeClient::new());

        let outcome = fire_trigger(
            &db,
            &kube,
            tid,
            json!({"symbol": "7203"}),
            TriggerSource::Hook,
        )
        .await
        .expect("fire ok");

        let task = strategy_task::Entity::find_by_id(outcome.task_id)
            .one(&db)
            .await
            .unwrap()
            .unwrap();
        let fired = trigger::Entity::find_by_id(tid)
            .one(&db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            (TaskShape::from(&task), TriggerFireShape::from(&fired)),
            (
                TaskShape {
                    strategy_id: sid,
                    source: "hook".to_string(),
                    prompt: "alert 7203".to_string(),
                    phase: crate::entities::sea_orm_active_enums::StrategyTaskPhase::Pending,
                },
                TriggerFireShape {
                    trigger_id: tid,
                    enabled: true,
                    kind: "hook".to_string(),
                    last_fired: true,
                },
            ),
        );
    }

    #[sqlx::test(migrations = false)]
    async fn fire_with_cron_source_writes_cron(pool: PgPool) {
        let db = create_test_db(pool).await;
        let sid = seed_strategy(&db, "s").await;
        let id = Uuid::new_v4();
        trigger::ActiveModel {
            trigger_id: Set(id),
            strategy_id: Set(sid),
            kind: Set("cron".to_string()),
            schedule: Set(Some("0 9 * * 1-5".to_string())),
            hook_slug: Set(None),
            event_match: Set(None),
            prompt_template: Set("morning {{strategy.name}}".to_string()),
            enabled: Set(true),
            last_fired_at: NotSet,
            created_at: NotSet,
            updated_at: NotSet,
        }
        .insert(&db)
        .await
        .unwrap();
        let kube: SharedKubeopencodeClient = Arc::new(FakeKubeopencodeClient::new());

        fire_trigger(&db, &kube, id, json!({}), TriggerSource::Cron)
            .await
            .expect("ok");

        let rows = strategy_task::Entity::find()
            .filter(strategy_task::Column::StrategyId.eq(sid))
            .all(&db)
            .await
            .unwrap();
        assert_eq!(
            rows.iter().map(TaskShape::from).collect::<Vec<_>>(),
            vec![TaskShape {
                strategy_id: sid,
                source: "cron".to_string(),
                prompt: "morning s".to_string(),
                phase: crate::entities::sea_orm_active_enums::StrategyTaskPhase::Pending,
            }],
        );
    }

    #[sqlx::test(migrations = false)]
    async fn fire_disabled_trigger_returns_error(pool: PgPool) {
        let db = create_test_db(pool).await;
        let sid = seed_strategy(&db, "s").await;
        let id = Uuid::new_v4();
        trigger::ActiveModel {
            trigger_id: Set(id),
            strategy_id: Set(sid),
            kind: Set("hook".to_string()),
            schedule: Set(None),
            hook_slug: Set(Some("off".to_string())),
            event_match: Set(None),
            prompt_template: Set("x".to_string()),
            enabled: Set(false),
            last_fired_at: NotSet,
            created_at: NotSet,
            updated_at: NotSet,
        }
        .insert(&db)
        .await
        .unwrap();
        let kube: SharedKubeopencodeClient = Arc::new(FakeKubeopencodeClient::new());

        let err = fire_trigger(&db, &kube, id, json!({}), TriggerSource::Hook)
            .await
            .expect_err("disabled");
        assert_eq!(err.to_string(), format!("trigger {id} is disabled"));
    }

    #[sqlx::test(migrations = false)]
    async fn fire_missing_trigger_returns_not_found(pool: PgPool) {
        let db = create_test_db(pool).await;
        let kube: SharedKubeopencodeClient = Arc::new(FakeKubeopencodeClient::new());
        let missing = Uuid::new_v4();
        let err = fire_trigger(&db, &kube, missing, json!({}), TriggerSource::Hook)
            .await
            .expect_err("not found");
        assert_eq!(err.to_string(), format!("trigger {missing} not found"));
    }

    #[sqlx::test(migrations = false)]
    async fn fire_does_not_update_last_fired_when_submit_fails(pool: PgPool) {
        let db = create_test_db(pool).await;
        // 戦略を Pending のまま投入 → submit_strategy_task が AgentPending で失敗する
        let sid = Uuid::new_v4();
        strategy::ActiveModel {
            id: Set(sid),
            name: Set("p".to_string()),
            description: Set(None),
            sort_order: Set(0),
            agents_md: NotSet,
            skills: NotSet,
            agent_status: NotSet,
            agent_error: NotSet,
            created_at: NotSet,
            updated_at: NotSet,
        }
        .insert(&db)
        .await
        .unwrap();
        let tid = seed_hook_trigger(&db, sid, "p", "x").await;
        let kube: SharedKubeopencodeClient = Arc::new(FakeKubeopencodeClient::new());

        let err = fire_trigger(&db, &kube, tid, json!({}), TriggerSource::Hook)
            .await
            .expect_err("submit fails");
        let row = trigger::Entity::find_by_id(tid)
            .one(&db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            (err.to_string(), TriggerFireShape::from(&row)),
            (
                "submit strategy task failed: strategy agent not ready: status=pending \
                 (reconcile in progress)"
                    .to_string(),
                TriggerFireShape {
                    trigger_id: tid,
                    enabled: true,
                    kind: "hook".to_string(),
                    last_fired: false,
                },
            ),
        );
    }
}
