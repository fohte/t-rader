use uuid::Uuid;

use crate::strategy_scope::StrategyScope;

use super::error::{GetTaskError, ListTasksError};
use super::types::{StrategyTask, StrategyTaskStep, TaskListQuery, TaskStatusView};
use super::{StrategyTaskRepositoryError, StrategyTaskUseCases};

impl StrategyTaskUseCases {
    pub async fn list(&self, query: TaskListQuery) -> Result<Vec<TaskStatusView>, ListTasksError> {
        self.repository
            .list(query)
            .await
            .map(|tasks| tasks.into_iter().map(status_view).collect())
            .map_err(Into::into)
    }

    pub async fn list_for_strategy(
        &self,
        scope: StrategyScope,
        purpose: Option<String>,
    ) -> Result<Vec<TaskStatusView>, ListTasksError> {
        self.list(TaskListQuery {
            strategy_id: Some(scope.id()),
            purpose,
        })
        .await
    }

    pub async fn get_for_strategy(
        &self,
        scope: StrategyScope,
        task_id: Uuid,
    ) -> Result<TaskStatusView, GetTaskError> {
        let task = self
            .repository
            .find_by_id(task_id)
            .await?
            .ok_or(GetTaskError::NotFound(task_id))?;
        if task.strategy_id != scope.id() {
            return Err(GetTaskError::StrategyMismatch {
                task_id,
                strategy_id: scope.id(),
            });
        }
        self.status_view_with_steps(task).await
    }

    pub async fn get_by_a2a_task_id(
        &self,
        a2a_task_id: &str,
    ) -> Result<Option<TaskStatusView>, GetTaskError> {
        let Some(task) = self.repository.find_by_a2a_task_id(a2a_task_id).await? else {
            return Ok(None);
        };
        self.status_view_with_steps(task).await.map(Some)
    }

    pub async fn list_in_flight_tasks(
        &self,
    ) -> Result<Vec<StrategyTask>, StrategyTaskRepositoryError> {
        self.repository.list_in_flight().await
    }

    async fn status_view_with_steps(
        &self,
        task: StrategyTask,
    ) -> Result<TaskStatusView, GetTaskError> {
        let steps = self.repository.list_steps(task.task_id).await?;
        let steps = steps.iter().map(step_to_wire_json).collect::<Vec<_>>();
        let mut view = status_view(task);
        view.steps = serde_json::Value::Array(steps);
        Ok(view)
    }
}

fn status_view(task: StrategyTask) -> TaskStatusView {
    TaskStatusView {
        task_id: task.task_id,
        strategy_id: task.strategy_id,
        a2a_task_id: task.a2a_task_id,
        source: task.source,
        prompt: task.prompt,
        phase: task.phase,
        error_summary: task.error_summary,
        result_text: task.result_text,
        created_at: task.created_at,
        updated_at: task.updated_at,
        steps: serde_json::json!([]),
        purpose: task.purpose,
        as_of: task.as_of,
    }
}

pub(super) fn step_to_wire_json(step: &StrategyTaskStep) -> serde_json::Value {
    let mut object = serde_json::Map::from_iter([
        ("phase_key".to_string(), serde_json::json!(step.phase_key)),
        ("label".to_string(), serde_json::json!(step.label)),
        ("model".to_string(), serde_json::json!(step.model)),
        (
            "status".to_string(),
            serde_json::json!(step.status.as_str()),
        ),
        (
            "started_at".to_string(),
            serde_json::json!(step.started_at.to_rfc3339()),
        ),
        ("trace_id".to_string(), serde_json::json!(step.trace_id)),
        ("span_id".to_string(), serde_json::json!(step.span_id)),
    ]);
    if let Some(item) = &step.item {
        object.insert("item".to_string(), item.clone());
    }
    if let Some(item_label) = &step.item_label {
        object.insert("item_label".to_string(), serde_json::json!(item_label));
    }
    if let Some(output) = &step.output {
        object.insert("output".to_string(), output.clone());
    }
    if let Some(finished_at) = step.finished_at {
        object.insert(
            "finished_at".to_string(),
            serde_json::json!(finished_at.to_rfc3339()),
        );
    }
    if let Some(error) = &step.error {
        object.insert("error".to_string(), serde_json::json!(error));
    }
    serde_json::Value::Object(object)
}
