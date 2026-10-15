use axum::Json;
use axum::extract::State;
use core_application::custom_indicator_evaluation::{
    CustomIndicatorEvaluationRepositoryError, CustomIndicatorEvaluationUseCaseError,
    CustomIndicatorEvaluationUseCases,
};
use uuid::Uuid;

use crate::error::{AppError, ErrorResponse};
use crate::extractors::JsonPath;
use crate::models::CustomIndicatorEvaluationResponse;

/// 自作指標の実行記録を取得
#[utoipa::path(
    get,
    path = "/api/custom-indicator-evaluations/{evaluation_id}",
    tag = "custom_indicators",
    params(("evaluation_id" = Uuid, Path, description = "自作指標の実行記録 ID")),
    responses(
        (status = 200, body = CustomIndicatorEvaluationResponse),
        (status = 400, description = "リクエストパラメータが不正", body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn get_custom_indicator_evaluation(
    State(use_cases): State<CustomIndicatorEvaluationUseCases>,
    JsonPath(evaluation_id): JsonPath<Uuid>,
) -> Result<Json<CustomIndicatorEvaluationResponse>, AppError> {
    let evaluation = use_cases
        .find_by_id(evaluation_id)
        .await
        .map_err(map_evaluation_error)?
        .ok_or_else(|| AppError::NotFound(format!("evaluation {evaluation_id} not found")))?;

    Ok(Json(evaluation.into()))
}

fn map_evaluation_error(error: CustomIndicatorEvaluationUseCaseError) -> AppError {
    match error {
        CustomIndicatorEvaluationUseCaseError::Repository(
            CustomIndicatorEvaluationRepositoryError::Database(error),
        ) => error.into(),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use axum::body::to_bytes;
    use axum::http::StatusCode;
    use axum::response::IntoResponse;
    use chrono::DateTime;
    use core_application::custom_indicator_evaluation::{
        CustomIndicatorEvaluation, CustomIndicatorEvaluationUseCases,
        FakeCustomIndicatorEvaluationRepository,
    };
    use serde_json::json;

    use super::get_custom_indicator_evaluation;
    use crate::extractors::JsonPath;
    use crate::models::CustomIndicatorEvaluationResponse;

    fn evaluation(id: uuid::Uuid) -> CustomIndicatorEvaluation {
        CustomIndicatorEvaluation {
            id,
            indicator_id: uuid::Uuid::from_u128(1),
            indicator_name: "デモ指標".to_owned(),
            code: "def calculate(args):\n    return {\"value\": args[\"value\"]}".to_owned(),
            args: json!({ "value": 12 }),
            output: json!({ "value": 12 }),
            evaluated_at: DateTime::parse_from_rfc3339("2026-01-02T03:04:05Z")
                .expect("valid timestamp"),
        }
    }

    fn use_cases(
        evaluation: Option<CustomIndicatorEvaluation>,
    ) -> CustomIndicatorEvaluationUseCases {
        let repository = Arc::new(FakeCustomIndicatorEvaluationRepository::new());
        if let Some(evaluation) = evaluation {
            repository.seed(evaluation);
        }
        CustomIndicatorEvaluationUseCases::new(repository)
    }

    #[tokio::test]
    async fn get_returns_the_evaluation_fields_used_by_the_evidence_card() {
        let id = uuid::Uuid::from_u128(2);
        let use_cases = use_cases(Some(evaluation(id)));

        let response =
            get_custom_indicator_evaluation(axum::extract::State(use_cases), JsonPath(id))
                .await
                .expect("evaluation exists");

        assert_eq!(
            response.0,
            CustomIndicatorEvaluationResponse {
                id,
                indicator_name: "デモ指標".to_owned(),
                code: "def calculate(args):\n    return {\"value\": args[\"value\"]}".to_owned(),
                args: json!({ "value": 12 }),
                output: json!({ "value": 12 }),
                evaluated_at: DateTime::parse_from_rfc3339("2026-01-02T03:04:05Z")
                    .expect("valid timestamp"),
            },
        );
    }

    #[tokio::test]
    async fn get_returns_not_found_when_the_evaluation_is_missing() {
        let id = uuid::Uuid::from_u128(3);
        let use_cases = use_cases(None);
        let error = get_custom_indicator_evaluation(axum::extract::State(use_cases), JsonPath(id))
            .await
            .expect_err("missing evaluation should return an error");
        let response = error.into_response();
        let status = response.status();
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("read response body");
        let body: serde_json::Value = serde_json::from_slice(&body).expect("parse response body");

        assert_eq!(
            (status, body),
            (
                StatusCode::NOT_FOUND,
                json!({ "error": format!("evaluation {id} not found") }),
            ),
        );
    }
}
