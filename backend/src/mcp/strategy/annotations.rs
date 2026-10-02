//! アノテーション操作の inner method 実装。
//!
//! 戦略境界の検証はユースケースが担う。

use core_application::annotation::{
    AnnotationListQuery, AnnotationReadQueryError, AnnotationReadUseCaseError,
    AnnotationUseCaseError, CreateAnnotationCommand,
};
use core_application::change_history::Actor;
use core_application::strategy_scope::StrategyScope;
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

/// 戦略 MCP の annotation は strategy 所属が必須なので、欠落時は明示的に失敗させる。
fn annotation_use_case_to_dto(
    m: core_application::annotation::Annotation,
) -> Result<AnnotationDto, McpError> {
    let strategy_id = m.strategy_id.ok_or_else(|| {
        internal_error(format!(
            "annotation {} has no strategy_id despite session scoping",
            m.id
        ))
    })?;
    Ok(AnnotationDto {
        annotation_id: m.id,
        strategy_id,
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
    })
}

impl StrategyServer {
    pub(crate) async fn create_annotation_inner(
        &self,
        scope: impl Into<StrategyScope>,
        execution_step_id: Option<Uuid>,
        execution_task_id: Option<String>,
        params: CreateAnnotationParams,
    ) -> Result<CreateAnnotationResult, McpError> {
        let scope = scope.into();
        let price = params.price.map(f64_to_decimal).transpose()?;
        let created = self
            .dependencies
            .annotations
            .create(CreateAnnotationCommand {
                scope: Some(scope),
                actor: Actor::Llm { label: "analyst" },
                strategy_id: Some(scope.id()),
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
            annotation: annotation_use_case_to_dto(created)?,
        })
    }

    pub(crate) async fn read_annotations_inner(
        &self,
        scope: impl Into<StrategyScope>,
        params: ReadAnnotationsParams,
    ) -> Result<ReadAnnotationsResult, McpError> {
        let query = AnnotationListQuery {
            strategy_id: None,
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
            .list_annotations(query, Some(scope.into()))
            .await
            .map_err(annotation_read_error_to_mcp)?;
        Ok(ReadAnnotationsResult {
            annotations: annotations
                .into_iter()
                .map(annotation_use_case_to_dto)
                .collect::<Result<Vec<_>, _>>()?,
        })
    }
}

pub(super) fn annotation_read_error_to_mcp(error: AnnotationReadUseCaseError) -> McpError {
    match error {
        AnnotationReadUseCaseError::NotFound(_) => {
            McpError::resource_not_found("annotation not found", None)
        }
        AnnotationReadUseCaseError::Forbidden(id) => invalid_params(format!(
            "forbidden: annotation {id} belongs to another strategy"
        )),
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
        AnnotationUseCaseError::ScopeMismatch => {
            invalid_params("annotation belongs to a different strategy")
        }
        other => {
            tracing::error!(error = %other, "strategy mcp annotation operation failed");
            internal_error(format!("database error: {other}"))
        }
    }
}
