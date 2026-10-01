//! cron trigger を 1 tick 評価する単発処理。

use std::time::Duration;

use core_application::trigger::TriggerUseCases;

use crate::agent_client::SharedAgentTaskClient;

pub const DEFAULT_INTERVAL: Duration = Duration::from_secs(60);

/// 1 tick の cron trigger 処理を application use case に委譲する。
pub async fn run_once(
    use_cases: &TriggerUseCases,
    agent_client: &SharedAgentTaskClient,
    interval: Duration,
) -> usize {
    use_cases
        .run_cron_tick(agent_client.as_ref(), interval)
        .await
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use chrono::TimeZone;
    use sea_orm::ActiveModelTrait;
    use sea_orm::ActiveValue::{NotSet, Set};
    use sea_orm::EntityTrait;
    use uuid::Uuid;

    use crate::agent_client::{FakeAgentTaskClient, SharedAgentTaskClient};
    use crate::services::strategy_tasks::DEFAULT_PURPOSE;
    use crate::services::use_cases::build_use_cases;
    use crate::testing::agent_config;
    use crate::testing::{insert_test_cron_trigger, insert_test_hook_trigger};
    use gateway_postgres::entities::sea_orm_active_enums::StrategyTaskPhase;
    use gateway_postgres::entities::{strategy, strategy_task};

    use super::*;

    async fn seed_strategy(db: &impl sea_orm::ConnectionTrait) -> Uuid {
        let id = Uuid::new_v4();
        strategy::ActiveModel {
            id: Set(id),
            name: Set("sample strategy".to_string()),
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

    #[derive(Debug, PartialEq, Eq)]
    struct TaskShape {
        strategy_id: Uuid,
        purpose: Option<String>,
        source: String,
        prompt: String,
        phase: StrategyTaskPhase,
    }

    impl From<strategy_task::Model> for TaskShape {
        fn from(row: strategy_task::Model) -> Self {
            Self {
                strategy_id: row.strategy_id,
                purpose: row.purpose,
                source: row.source,
                prompt: row.prompt,
                phase: row.phase,
            }
        }
    }

    #[backend_test_macros::database_test]
    async fn fires_due_cron_and_writes_strategy_task(db: gateway_postgres::DatabaseHandle) {
        let strategy_id = seed_strategy(&db).await;
        agent_config::create(&db, DEFAULT_PURPOSE.to_string())
            .await
            .expect("insert test agent_config");
        let past = chrono::Utc.with_ymd_and_hms(2000, 1, 1, 0, 0, 0).unwrap();
        let trigger_id = insert_test_cron_trigger(
            &db,
            strategy_id,
            "* * * * *",
            true,
            Some(past),
            "{{strategy.name}} morning",
            None,
        )
        .await;
        let agent_client: SharedAgentTaskClient = Arc::new(FakeAgentTaskClient::new());
        let use_cases = build_use_cases(db.clone());
        let triggers = use_cases.triggers();

        let attempts = run_once(&triggers, &agent_client, DEFAULT_INTERVAL).await;

        let tasks = strategy_task::Entity::find().all(&db).await.unwrap();
        let fired = triggers.get(trigger_id).await.unwrap();
        assert_eq!(
            (
                attempts,
                tasks.into_iter().map(TaskShape::from).collect::<Vec<_>>(),
                fired.last_fired_at.is_some(),
            ),
            (
                1,
                vec![TaskShape {
                    strategy_id,
                    purpose: Some(DEFAULT_PURPOSE.to_string()),
                    source: "cron".to_string(),
                    prompt: "sample strategy morning".to_string(),
                    phase: StrategyTaskPhase::Running,
                }],
                true,
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn fires_due_cron_with_trigger_purpose(db: gateway_postgres::DatabaseHandle) {
        let strategy_id = seed_strategy(&db).await;
        agent_config::create(&db, "synthetic-purpose".to_string())
            .await
            .expect("insert test agent_config");
        let past = chrono::Utc.with_ymd_and_hms(2000, 1, 1, 0, 0, 0).unwrap();
        insert_test_cron_trigger(
            &db,
            strategy_id,
            "* * * * *",
            true,
            Some(past),
            "{{strategy.name}} morning",
            Some("synthetic-purpose"),
        )
        .await;

        let agent_client: SharedAgentTaskClient = Arc::new(FakeAgentTaskClient::new());
        let triggers = build_use_cases(db.clone()).triggers();
        let attempts = run_once(&triggers, &agent_client, DEFAULT_INTERVAL).await;
        let tasks = strategy_task::Entity::find()
            .all(&db)
            .await
            .unwrap()
            .into_iter()
            .map(TaskShape::from)
            .collect::<Vec<_>>();

        assert_eq!(
            (attempts, tasks),
            (
                1,
                vec![TaskShape {
                    strategy_id,
                    purpose: Some("synthetic-purpose".to_string()),
                    source: "cron".to_string(),
                    prompt: "sample strategy morning".to_string(),
                    phase: StrategyTaskPhase::Running,
                }],
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn skips_disabled_cron(db: gateway_postgres::DatabaseHandle) {
        let strategy_id = seed_strategy(&db).await;
        let past = chrono::Utc.with_ymd_and_hms(2000, 1, 1, 0, 0, 0).unwrap();
        insert_test_cron_trigger(&db, strategy_id, "* * * * *", false, Some(past), "x", None).await;
        let agent_client: SharedAgentTaskClient = Arc::new(FakeAgentTaskClient::new());
        let use_cases = build_use_cases(db.clone());
        let triggers = use_cases.triggers();

        let attempts = run_once(&triggers, &agent_client, DEFAULT_INTERVAL).await;

        let tasks = strategy_task::Entity::find()
            .all(&db)
            .await
            .unwrap()
            .into_iter()
            .map(TaskShape::from)
            .collect::<Vec<_>>();
        assert_eq!((attempts, tasks), (0, Vec::new()));
    }

    #[backend_test_macros::database_test]
    async fn skips_when_no_slot_after_last_fire(db: gateway_postgres::DatabaseHandle) {
        let strategy_id = seed_strategy(&db).await;
        let just_fired = chrono::Utc::now() - chrono::Duration::seconds(1);
        insert_test_cron_trigger(
            &db,
            strategy_id,
            "0 9 * * *",
            true,
            Some(just_fired),
            "x",
            None,
        )
        .await;
        let agent_client: SharedAgentTaskClient = Arc::new(FakeAgentTaskClient::new());
        let use_cases = build_use_cases(db.clone());
        let triggers = use_cases.triggers();

        let attempts = run_once(&triggers, &agent_client, DEFAULT_INTERVAL).await;

        let tasks = strategy_task::Entity::find()
            .all(&db)
            .await
            .unwrap()
            .into_iter()
            .map(TaskShape::from)
            .collect::<Vec<_>>();
        assert_eq!((attempts, tasks), (0, Vec::new()));
    }

    #[backend_test_macros::database_test]
    async fn ignores_hook_kind(db: gateway_postgres::DatabaseHandle) {
        let strategy_id = seed_strategy(&db).await;
        insert_test_hook_trigger(&db, strategy_id, "sample-hook", "x", None, true).await;
        let agent_client: SharedAgentTaskClient = Arc::new(FakeAgentTaskClient::new());
        let use_cases = build_use_cases(db.clone());
        let triggers = use_cases.triggers();

        let attempts = run_once(&triggers, &agent_client, DEFAULT_INTERVAL).await;

        let tasks = strategy_task::Entity::find()
            .all(&db)
            .await
            .unwrap()
            .into_iter()
            .map(TaskShape::from)
            .collect::<Vec<_>>();
        assert_eq!((attempts, tasks), (0, Vec::new()));
    }
}
