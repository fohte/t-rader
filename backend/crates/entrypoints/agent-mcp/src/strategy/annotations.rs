//! アノテーション操作の inner method 実装。
//!
//! 作成・読み取りともに戦略をまたいで扱う。

use core_application::annotation::{
    AnnotationListQuery, AnnotationReadQueryError, AnnotationReadUseCaseError,
    AnnotationUseCaseError, CreateAnnotationCommand,
};
use core_application::change_history::Actor;
use rmcp::ErrorData as McpError;
use rust_decimal::Decimal;
use uuid::Uuid;

use super::dto::{
    AnnotationDto, CreateAnnotationParams, CreateAnnotationResult, ReadAnnotationsParams,
    ReadAnnotationsResult,
};
use super::{
    DEFAULT_ANNOTATION_STATUS, STRATEGY_AGENT_ACTOR, StrategyServer, clamp_limit, decimal_to_f64,
    internal_error, invalid_params,
};

fn f64_to_decimal(v: f64) -> Result<Decimal, McpError> {
    Decimal::try_from(v).map_err(|err| invalid_params(format!("invalid decimal value: {err}")))
}

fn annotation_use_case_to_dto(m: core_application::annotation::Annotation) -> AnnotationDto {
    AnnotationDto {
        annotation_id: m.id,
        target_symbol: m.target_symbol,
        target_kind: m.target_kind,
        timestamp: m.timestamp,
        price: m.price.map(decimal_to_f64),
        text: m.text,
        status: m.status,
        linked_note_id: m.linked_note_id,
        created_by_kind: m.created_by_kind,
        created_at: m.created_at,
        updated_at: m.updated_at,
    }
}

impl StrategyServer {
    pub(crate) async fn create_annotation_inner(
        &self,
        execution_step_id: Option<Uuid>,
        execution_task_id: Option<String>,
        params: CreateAnnotationParams,
    ) -> Result<CreateAnnotationResult, McpError> {
        let price = params.price.map(f64_to_decimal).transpose()?;
        let created = self
            .dependencies
            .annotations
            .create(CreateAnnotationCommand {
                actor: Actor::Llm { label: "analyst" },
                target_symbol: params.target_symbol,
                target_kind: params.target_kind,
                timestamp: params.timestamp,
                price,
                text: params.text,
                status: DEFAULT_ANNOTATION_STATUS.into(),
                linked_note_id: params.linked_note_id,
                created_by_kind: STRATEGY_AGENT_ACTOR.into(),
                execution_step_id,
                execution_task_id,
            })
            .await
            .map_err(annotation_use_case_error)?;
        Ok(CreateAnnotationResult {
            annotation: annotation_use_case_to_dto(created),
        })
    }

    pub(crate) async fn read_annotations_inner(
        &self,
        params: ReadAnnotationsParams,
    ) -> Result<ReadAnnotationsResult, McpError> {
        let query = AnnotationListQuery {
            target_symbol: params
                .target_symbol
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string),
            limit: Some(clamp_limit(params.limit)),
        };
        let annotations = self
            .dependencies
            .annotation_reads
            .list_annotations(query)
            .await
            .map_err(annotation_read_error_to_mcp)?;
        Ok(ReadAnnotationsResult {
            annotations: annotations
                .into_iter()
                .map(annotation_use_case_to_dto)
                .collect(),
        })
    }
}

pub(super) fn annotation_read_error_to_mcp(error: AnnotationReadUseCaseError) -> McpError {
    match error {
        AnnotationReadUseCaseError::NotFound(_) => {
            McpError::resource_not_found("annotation not found", None)
        }
        AnnotationReadUseCaseError::Query(AnnotationReadQueryError::Database(error)) => {
            super::persistence_error_to_mcp(error)
        }
    }
}

fn annotation_use_case_error(error: AnnotationUseCaseError) -> McpError {
    match error {
        AnnotationUseCaseError::Validation(message) => invalid_params(message),
        AnnotationUseCaseError::NotFound(id) => {
            McpError::resource_not_found(format!("annotation {id} not found"), None)
        }
        AnnotationUseCaseError::LinkedNoteNotFound(_) => {
            McpError::resource_not_found("note not found", None)
        }
        other => {
            tracing::error!(error = %other, "strategy mcp annotation operation failed");
            internal_error(format!("database error: {other}"))
        }
    }
}

#[cfg(test)]
mod tests {
    use chrono::DateTime;
    use core_application::annotation::Annotation;
    use uuid::Uuid;

    use super::{AnnotationDto, annotation_use_case_to_dto};

    #[test]
    fn annotation_dto_maps_annotation_fields() {
        let id = Uuid::nil();
        let timestamp =
            DateTime::parse_from_rfc3339("2000-01-01T00:00:00+00:00").expect("valid timestamp");
        let actual = annotation_use_case_to_dto(Annotation {
            id,
            target_symbol: "demo-code".into(),
            target_kind: "stock".into(),
            timestamp,
            price: None,
            text: "Example annotation".into(),
            status: "unread".into(),
            linked_note_id: None,
            created_by_kind: "human".into(),
            created_at: timestamp,
            updated_at: timestamp,
            execution_step_id: None,
            execution_task_id: None,
        });

        assert_eq!(
            actual,
            AnnotationDto {
                annotation_id: id,
                target_symbol: "demo-code".into(),
                target_kind: "stock".into(),
                timestamp,
                price: None,
                text: "Example annotation".into(),
                status: "unread".into(),
                linked_note_id: None,
                created_by_kind: "human".into(),
                created_at: timestamp,
                updated_at: timestamp,
            }
        );
    }
}
