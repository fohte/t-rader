use async_trait::async_trait;
use core_application::mcp_tool_call_count::{
    McpToolCallCountRepository, McpToolCallCountRepositoryError,
};
use sea_orm::ActiveValue::{NotSet, Set};
use sea_orm::sea_query::{Expr, OnConflict};
use sea_orm::{ColumnTrait, EntityTrait, ExprTrait, QueryFilter};

use crate::DatabaseHandle;
use crate::entities::mcp_tool_call_count;
use crate::persistence::persistence_error;

#[derive(Clone)]
pub struct PostgresMcpToolCallCountRepository {
    db: DatabaseHandle,
}

impl PostgresMcpToolCallCountRepository {
    pub fn new(db: DatabaseHandle) -> Self {
        Self { db }
    }
}

#[async_trait]
impl McpToolCallCountRepository for PostgresMcpToolCallCountRepository {
    async fn increment(
        &self,
        task_execution_id: &str,
        tool_name: &str,
    ) -> Result<i32, McpToolCallCountRepositoryError> {
        let model = mcp_tool_call_count::ActiveModel {
            id: NotSet,
            task_execution_id: Set(task_execution_id.to_string()),
            tool_name: Set(tool_name.to_string()),
            call_count: Set(1),
            created_at: NotSet,
            updated_at: NotSet,
        };
        let row = mcp_tool_call_count::Entity::insert(model)
            .on_conflict(
                OnConflict::columns([
                    mcp_tool_call_count::Column::TaskExecutionId,
                    mcp_tool_call_count::Column::ToolName,
                ])
                .value(
                    mcp_tool_call_count::Column::CallCount,
                    Expr::col((
                        mcp_tool_call_count::Entity,
                        mcp_tool_call_count::Column::CallCount,
                    ))
                    .add(1),
                )
                .update_column(mcp_tool_call_count::Column::UpdatedAt)
                .to_owned(),
            )
            .exec_with_returning(&self.db)
            .await
            .map_err(repository_error)?;
        Ok(row.call_count)
    }

    async fn decrement(
        &self,
        task_execution_id: &str,
        tool_name: &str,
    ) -> Result<(), McpToolCallCountRepositoryError> {
        mcp_tool_call_count::Entity::update_many()
            .col_expr(
                mcp_tool_call_count::Column::CallCount,
                Expr::col(mcp_tool_call_count::Column::CallCount).sub(1),
            )
            .col_expr(
                mcp_tool_call_count::Column::UpdatedAt,
                Expr::current_timestamp(),
            )
            .filter(mcp_tool_call_count::Column::TaskExecutionId.eq(task_execution_id))
            .filter(mcp_tool_call_count::Column::ToolName.eq(tool_name))
            .exec(&self.db)
            .await
            .map(|_| ())
            .map_err(repository_error)
    }
}

fn repository_error(error: sea_orm::DbErr) -> McpToolCallCountRepositoryError {
    McpToolCallCountRepositoryError::Database(persistence_error(error))
}

#[cfg(test)]
mod tests {
    use super::PostgresMcpToolCallCountRepository;
    use core_application::mcp_tool_call_count::McpToolCallCountRepository;

    #[backend_test_macros::database_test]
    async fn repository_increments_each_task_and_tool_independently(db: crate::DatabaseHandle) {
        let repository = PostgresMcpToolCallCountRepository::new(db);
        let task_a_first = repository
            .increment("fictional-task-a", "search_web")
            .await
            .expect("increment");
        let task_a_second = repository
            .increment("fictional-task-a", "search_web")
            .await
            .expect("increment");
        let task_a_other_tool = repository
            .increment("fictional-task-a", "fictional-tool")
            .await
            .expect("increment");
        let task_b_first = repository
            .increment("fictional-task-b", "search_web")
            .await
            .expect("increment");

        assert_eq!(
            (task_a_first, task_a_second, task_a_other_tool, task_b_first,),
            (1, 2, 1, 1),
        );
    }

    #[backend_test_macros::database_test]
    async fn repository_decrements_a_reservation(db: crate::DatabaseHandle) {
        let repository = PostgresMcpToolCallCountRepository::new(db);
        repository
            .increment("fictional-task", "search_web")
            .await
            .expect("reserve");
        repository
            .decrement("fictional-task", "search_web")
            .await
            .expect("release");
        let call_count = repository
            .increment("fictional-task", "search_web")
            .await
            .expect("reserve again");

        assert_eq!(call_count, 1);
    }
}
