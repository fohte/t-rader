//! 管理 MCP の統合テストで共有するヘルパー。

use std::sync::Arc;

use sea_orm::ActiveModelTrait;
use sea_orm::ActiveValue::Set;
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use crate::agent_client::{FakeAgentTaskClient, SharedAgentTaskClient};
use crate::entities::strategy;

use super::MgmtServer;

pub(super) async fn insert_strategy(db: &DatabaseConnection, name: &str) -> Uuid {
    let id = Uuid::new_v4();
    strategy::ActiveModel {
        id: Set(id),
        name: Set(name.to_string()),
        description: Set(None),
        sort_order: Set(0),
        agents_md: sea_orm::ActiveValue::NotSet,
        skills: sea_orm::ActiveValue::NotSet,
        agent_graph: sea_orm::ActiveValue::NotSet,
        created_at: sea_orm::ActiveValue::NotSet,
        updated_at: sea_orm::ActiveValue::NotSet,
    }
    .insert(db)
    .await
    .unwrap();
    id
}

pub(super) fn build_server(db: DatabaseConnection, fake: Arc<FakeAgentTaskClient>) -> MgmtServer {
    MgmtServer::new(db, fake as SharedAgentTaskClient)
}
