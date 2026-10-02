//! 管理 MCP の統合テストで共有するヘルパー。

use std::sync::Arc;

use sea_orm::ActiveModelTrait;
use sea_orm::ActiveValue::Set;
use uuid::Uuid;

use crate::agent_client::FakeAgentTaskClient;
use core_application::agent_task_client::SharedAgentTaskClient;
use gateway_postgres::entities::strategy;

use super::{MgmtDependencies, MgmtServer};

pub(super) async fn insert_strategy(db: &impl sea_orm::ConnectionTrait, name: &str) -> Uuid {
    let id = Uuid::new_v4();
    strategy::ActiveModel {
        id: Set(id),
        name: Set(name.to_string()),
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

pub(super) fn build_server(
    db: impl Into<gateway_postgres::DatabaseHandle>,
    fake: Arc<FakeAgentTaskClient>,
) -> MgmtServer {
    let use_cases = crate::services::use_cases::build_use_cases(db);
    MgmtServer::new(MgmtDependencies {
        strategies: use_cases.strategies(),
        strategy_scope: Arc::new(use_cases.strategy_scope()),
        strategy_tasks: use_cases.strategy_tasks(),
        triggers: use_cases.triggers(),
        note_kinds: use_cases.note_kinds(),
        note_reads: use_cases.note_reads(),
        annotation_reads: use_cases.annotation_reads(),
        rss_feeds: use_cases.rss_feeds(),
        agent_client: fake as SharedAgentTaskClient,
    })
}
