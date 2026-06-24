//! 戦略 Agent reconcile の orchestration。
//!
//! handler / mgmt MCP / 起動時 sweep から呼ばれる。DB から戦略行を読み、
//! `KubeopencodeClient::reconcile_strategy_agent` を実行し、結果を
//! `strategy.agent_status` / `strategy.agent_error` に書き戻す。

use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex as StdMutex};

use sea_orm::ActiveValue::Set;
use sea_orm::{ActiveModelTrait, DatabaseConnection, EntityTrait, IntoActiveModel};
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::entities::sea_orm_active_enums::StrategyAgentStatus;
use crate::entities::strategy;
use crate::kubeopencode::{KubeopencodeError, SharedKubeopencodeClient, StrategyAgentSpec};

/// `strategy_id` ごとに 1 個の `tokio::Mutex` を発行するレジストリ。
/// 同一戦略への reconcile を直列化し、古い SSA が新しい SSA を上書きする race を防ぐ。
///
/// 単一 backend replica 前提。multi-replica 化したら DB レベルの楽観ロック
/// (`agent_spec_version` 等) に昇格させること。
fn reconcile_lock_for(strategy_id: Uuid) -> Arc<Mutex<()>> {
    static REGISTRY: std::sync::OnceLock<StdMutex<HashMap<Uuid, Arc<Mutex<()>>>>> =
        std::sync::OnceLock::new();
    let registry = REGISTRY.get_or_init(|| StdMutex::new(HashMap::new()));
    let mut guard = registry.lock().unwrap_or_else(|e| e.into_inner());
    guard
        .entry(strategy_id)
        .or_insert_with(|| Arc::new(Mutex::new(())))
        .clone()
}

#[derive(Debug, thiserror::Error)]
pub enum StrategyAgentError {
    #[error("strategy {0} not found")]
    NotFound(Uuid),
    #[error("database error: {0}")]
    Database(#[from] sea_orm::DbErr),
}

/// `strategy.skills` (jsonb) を `{name: body}` に正規化する。
///
/// 想定スキーマ: top-level object で値が文字列。型が違うエントリは warn して捨てる。
fn parse_skills(value: &serde_json::Value) -> BTreeMap<String, String> {
    let Some(map) = value.as_object() else {
        return BTreeMap::new();
    };
    let mut out = BTreeMap::new();
    for (name, v) in map {
        if let Some(body) = v.as_str() {
            out.insert(name.clone(), body.to_string());
        } else {
            tracing::warn!(skill = %name, "strategy.skills entry is not a string; skipped");
        }
    }
    out
}

pub fn build_spec_from_model(row: &strategy::Model) -> StrategyAgentSpec {
    StrategyAgentSpec::new(row.id, row.agents_md.clone(), parse_skills(&row.skills))
}

/// 1 戦略分の reconcile を同期実行し、結果を DB に書き戻す。
///
/// reconcile 失敗時は `agent_status=failed` + `agent_error` を保存して `Ok(())` を返す。
/// `Err` は DB 自体のエラー (戦略 not found 等) のみ。
pub async fn reconcile_and_persist(
    db: &DatabaseConnection,
    kube: &SharedKubeopencodeClient,
    strategy_id: Uuid,
) -> Result<(), StrategyAgentError> {
    // 同一 strategy_id の reconcile を直列化する: 多重 spawn による古い SSA が新しい SSA を
    // 上書きする race を抑える。読み出し → kube apply → DB 書き戻し全体をロック内で実行する。
    let lock = reconcile_lock_for(strategy_id);
    let _guard = lock.lock().await;

    let row = strategy::Entity::find_by_id(strategy_id)
        .one(db)
        .await?
        .ok_or(StrategyAgentError::NotFound(strategy_id))?;

    let spec = build_spec_from_model(&row);
    let outcome = kube.reconcile_strategy_agent(&spec).await;

    // dev opt-out (KUBEOPENCODE_API_URL=disabled): apply 自体が走らないので状態を書き換えない
    if matches!(outcome, Err(KubeopencodeError::NotConfigured)) {
        return Ok(());
    }

    // reconcile 中に DELETE /api/strategies/{id} が走った race: row が消えていれば apply 済みの
    // Agent CR を後追いで掃除し、状態書き戻しは諦める (孤児リソース防止)
    let still_present = strategy::Entity::find_by_id(strategy_id)
        .one(db)
        .await?
        .is_some();
    if !still_present {
        if outcome.is_ok()
            && let Err(cleanup) = kube.delete_strategy_agent(&spec.agent_name).await
        {
            tracing::warn!(
                error = %cleanup,
                strategy_id = %strategy_id,
                "failed to clean up orphaned agent after concurrent strategy delete",
            );
        }
        return Ok(());
    }

    let mut active = row.into_active_model();
    match outcome {
        Ok(()) => {
            active.agent_status = Set(StrategyAgentStatus::Ready);
            active.agent_error = Set(None);
        }
        Err(err) => {
            tracing::warn!(
                error = %err,
                strategy_id = %strategy_id,
                "strategy agent reconcile failed",
            );
            active.agent_status = Set(StrategyAgentStatus::Failed);
            active.agent_error = Set(Some(err.to_string()));
        }
    }
    active.updated_at = Set(chrono::Utc::now().fixed_offset());
    active.update(db).await?;
    Ok(())
}

/// handler から fire-and-forget で呼ぶ用の wrapper。tokio::spawn して reconcile を回す。
pub fn spawn_reconcile(db: DatabaseConnection, kube: SharedKubeopencodeClient, strategy_id: Uuid) {
    tokio::spawn(async move {
        if let Err(err) = reconcile_and_persist(&db, &kube, strategy_id).await {
            tracing::warn!(
                error = %err,
                strategy_id = %strategy_id,
                "strategy agent reconcile orchestration error",
            );
        }
    });
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use sea_orm::ActiveValue::NotSet;
    use sqlx::PgPool;

    use crate::entities::strategy;
    use crate::kubeopencode::{FakeKubeopencodeClient, KubeopencodeError};
    use crate::testing::create_test_db;

    use super::*;

    async fn insert_strategy(db: &DatabaseConnection, name: &str) -> Uuid {
        let id = Uuid::new_v4();
        strategy::ActiveModel {
            id: Set(id),
            name: Set(name.to_string()),
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

    #[sqlx::test(migrations = false)]
    async fn reconcile_marks_row_ready_on_success(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_id = insert_strategy(&db, "x").await;
        let fake = Arc::new(FakeKubeopencodeClient::new());
        let kube: SharedKubeopencodeClient = fake.clone();

        reconcile_and_persist(&db, &kube, strategy_id)
            .await
            .expect("ok");

        let row = strategy::Entity::find_by_id(strategy_id)
            .one(&db)
            .await
            .unwrap()
            .unwrap();
        let reconciled: Vec<Uuid> = fake
            .reconciled
            .lock()
            .await
            .iter()
            .map(|s| s.strategy_id)
            .collect();
        assert_eq!(
            (row.agent_status, row.agent_error, reconciled),
            (StrategyAgentStatus::Ready, None, vec![strategy_id]),
        );
    }

    #[sqlx::test(migrations = false)]
    async fn reconcile_marks_row_failed_with_error_message(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_id = insert_strategy(&db, "x").await;
        let fake = Arc::new(FakeKubeopencodeClient::new());
        fake.set_reconcile_error(KubeopencodeError::Api {
            status: 500,
            message: "boom".into(),
        })
        .await;
        let kube: SharedKubeopencodeClient = fake;

        reconcile_and_persist(&db, &kube, strategy_id)
            .await
            .expect("ok");

        let row = strategy::Entity::find_by_id(strategy_id)
            .one(&db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            (row.agent_status, row.agent_error.as_deref()),
            (
                StrategyAgentStatus::Failed,
                Some("kubeopencode api error (status 500): boom"),
            ),
        );
    }

    #[sqlx::test(migrations = false)]
    async fn reconcile_is_idempotent_across_calls(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_id = insert_strategy(&db, "x").await;
        let fake = Arc::new(FakeKubeopencodeClient::new());
        let kube: SharedKubeopencodeClient = fake.clone();

        reconcile_and_persist(&db, &kube, strategy_id)
            .await
            .unwrap();
        reconcile_and_persist(&db, &kube, strategy_id)
            .await
            .unwrap();

        assert_eq!(fake.reconciled.lock().await.len(), 2);
        let row = strategy::Entity::find_by_id(strategy_id)
            .one(&db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(row.agent_status, StrategyAgentStatus::Ready);
    }

    #[sqlx::test(migrations = false)]
    async fn reconcile_leaves_state_pending_when_kube_disabled(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_id = insert_strategy(&db, "x").await;
        let fake = Arc::new(FakeKubeopencodeClient::new());
        fake.set_reconcile_error(KubeopencodeError::NotConfigured)
            .await;
        let kube: SharedKubeopencodeClient = fake;

        reconcile_and_persist(&db, &kube, strategy_id)
            .await
            .unwrap();

        let row = strategy::Entity::find_by_id(strategy_id)
            .one(&db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            (row.agent_status, row.agent_error),
            (StrategyAgentStatus::Pending, None),
        );
    }

    #[sqlx::test(migrations = false)]
    async fn concurrent_reconciles_for_same_strategy_are_serialized(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_id = insert_strategy(&db, "x").await;
        let fake = Arc::new(FakeKubeopencodeClient::new());
        let kube: SharedKubeopencodeClient = fake.clone();

        // 同一 strategy_id に対して 4 並列で reconcile を走らせる
        let mut handles = Vec::new();
        for _ in 0..4 {
            let db = db.clone();
            let kube = kube.clone();
            handles.push(tokio::spawn(async move {
                reconcile_and_persist(&db, &kube, strategy_id).await
            }));
        }
        for h in handles {
            h.await.unwrap().unwrap();
        }

        // 全 reconcile が完走し、最終状態は Ready
        let row = strategy::Entity::find_by_id(strategy_id)
            .one(&db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            (row.agent_status, fake.reconciled.lock().await.len()),
            (StrategyAgentStatus::Ready, 4),
        );
    }

    #[sqlx::test(migrations = false)]
    async fn reconcile_returns_err_when_strategy_missing(pool: PgPool) {
        let db = create_test_db(pool).await;
        let fake: SharedKubeopencodeClient = Arc::new(FakeKubeopencodeClient::new());
        let err = reconcile_and_persist(&db, &fake, Uuid::new_v4())
            .await
            .expect_err("missing strategy");
        assert!(matches!(err, StrategyAgentError::NotFound(_)));
    }

    #[test]
    fn parse_skills_filters_non_string_values() {
        let value = serde_json::json!({ "a": "body", "b": 42, "c": "x" });
        assert_eq!(
            parse_skills(&value),
            BTreeMap::from([
                ("a".to_string(), "body".to_string()),
                ("c".to_string(), "x".to_string()),
            ]),
        );
    }
}
