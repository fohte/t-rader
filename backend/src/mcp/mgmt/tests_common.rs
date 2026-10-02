//! 管理 MCP の統合テストで共有するヘルパー。

use std::sync::Arc;

use sea_orm::ActiveModelTrait;
use sea_orm::ActiveValue::Set;
use uuid::Uuid;

use crate::agent_client::FakeAgentTaskClient;
use gateway_postgres::entities::strategy;

use super::MgmtServer;

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
    MgmtServer::new(crate::mcp::mgmt_dependencies(
        &use_cases,
        fake as core_application::agent_task_client::SharedAgentTaskClient,
    ))
}
