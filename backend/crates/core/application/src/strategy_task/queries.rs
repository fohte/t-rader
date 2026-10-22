use uuid::Uuid;

use crate::strategy_scope::StrategyScope;

use super::error::{GetTaskError, ListTasksError};
use super::types::{
    StrategyTask, StrategyTaskContext, StrategyTaskOrigin, StrategyTaskStep, TaskListQuery,
    TaskStatusView,
};
use super::{StrategyTaskRepositoryError, StrategyTaskUseCases};

impl StrategyTaskUseCases {
    pub async fn find_origin_by_execution_step_id(
        &self,
        execution_step_id: Uuid,
    ) -> Result<Option<StrategyTaskOrigin>, GetTaskError> {
        self.repository
            .find_origin_by_execution_step_id(execution_step_id)
            .await
            .map_err(Into::into)
    }

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
        context: Option<StrategyTaskContext>,
    ) -> Result<Vec<TaskStatusView>, ListTasksError> {
        self.list(TaskListQuery {
            strategy_id: Some(scope.id()),
            purpose,
            context,
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
        context_kind: task.context_kind,
        context_id: task.context_id,
    }
}

pub(super) fn step_to_wire_json(step: &StrategyTaskStep) -> serde_json::Value {
    let mut object = serde_json::Map::from_iter([
        ("phase_key".to_string(), serde_json::json!(step.phase_key)),
        ("kind".to_string(), serde_json::json!(step.kind.as_str())),
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
    if let Some(item_key) = &step.item_key {
        object.insert("item_key".to_string(), item_key.clone());
    }
    if let Some(item_label) = &step.item_label {
        object.insert("item_label".to_string(), serde_json::json!(item_label));
    }
    if let Some(input_ref) = &step.input_ref {
        object.insert("input_ref".to_string(), input_ref.clone());
    }
    if let Some(output_ref) = &step.output_ref {
        object.insert("output_ref".to_string(), output_ref.clone());
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

#[cfg(test)]
mod tests {
    use chrono::DateTime;
    use uuid::Uuid;

    use crate::strategy_task::{StrategyTaskStep, StrategyTaskStepKind, StrategyTaskStepStatus};

    use super::step_to_wire_json;

    #[test]
    fn operation_step_wire_includes_nullable_model_and_payload_references() {
        let started_at =
            DateTime::parse_from_rfc3339("2031-04-05T06:07:08Z").expect("parse start time");
        let finished_at =
            DateTime::parse_from_rfc3339("2031-04-05T06:07:09Z").expect("parse finish time");
        let actual = step_to_wire_json(&StrategyTaskStep {
            execution_step_id: Uuid::from_u128(0x461),
            task_id: Uuid::from_u128(0x462),
            phase_key: "fictional-operation".to_string(),
            kind: StrategyTaskStepKind::Operation,
            label: "Fictional operation".to_string(),
            model: None,
            status: StrategyTaskStepStatus::Completed,
            item: None,
            item_key: Some(serde_json::json!({
                "payload_id": "00000000-0000-0000-0000-000000000463",
                "pointer": "/items",
                "index": 3,
            })),
            item_label: None,
            input_ref: Some(serde_json::json!({
                "payload_id": "00000000-0000-0000-0000-000000000464",
                "pointer": "/input",
            })),
            output_ref: Some(serde_json::json!({
                "payload_id": "00000000-0000-0000-0000-000000000465",
                "pointer": "/value",
            })),
            output: Some(serde_json::json!({ "count": 1 })),
            started_at,
            finished_at: Some(finished_at),
            trace_id: "fictional-trace".to_string(),
            span_id: "fictional-span".to_string(),
            error: None,
            seq: 2,
        });

        assert_eq!(
            actual,
            serde_json::json!({
                "phase_key": "fictional-operation",
                "kind": "operation",
                "label": "Fictional operation",
                "model": null,
                "status": "completed",
                "started_at": "2031-04-05T06:07:08+00:00",
                "trace_id": "fictional-trace",
                "span_id": "fictional-span",
                "item_key": {
                    "payload_id": "00000000-0000-0000-0000-000000000463",
                    "pointer": "/items",
                    "index": 3,
                },
                "input_ref": {
                    "payload_id": "00000000-0000-0000-0000-000000000464",
                    "pointer": "/input",
                },
                "output_ref": {
                    "payload_id": "00000000-0000-0000-0000-000000000465",
                    "pointer": "/value",
                },
                "output": { "count": 1 },
                "finished_at": "2031-04-05T06:07:09+00:00",
            }),
        );
    }
}
