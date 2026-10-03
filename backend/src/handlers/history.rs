use axum::Json;
use axum::extract::State;
use core_application::change_history::ChangeHistoryUseCaseError;
use serde::Deserialize;
use utoipa::IntoParams;
use uuid::Uuid;

use crate::AppState;
use crate::error::{AppError, ErrorResponse};
use crate::extractors::{JsonPath, JsonQuery};
use crate::models::ChangeHistoryResponse;

fn map_err(error: ChangeHistoryUseCaseError) -> AppError {
    match error {
        ChangeHistoryUseCaseError::NotFound(id) => {
            AppError::NotFound(format!("history {id} not found"))
        }
        ChangeHistoryUseCaseError::Query(
            core_application::change_history::ChangeHistoryQueryError::Database(error),
        ) => error.into(),
    }
}

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ListHistoryQuery {
    pub target_kind: Option<String>,
    pub target_id: Option<Uuid>,
    /// 1〜500 (デフォルト 100)
    pub limit: Option<u64>,
}

/// 変更履歴一覧。target で絞らない場合は全体の最新を返す。
#[utoipa::path(
    get,
    path = "/api/history",
    tag = "history",
    params(ListHistoryQuery),
    responses(
        (status = 200, body = Vec<ChangeHistoryResponse>),
        (status = 400, description = "リクエストパラメータが不正", body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn list_history(
    State(state): State<AppState>,
    JsonQuery(p): JsonQuery<ListHistoryQuery>,
) -> Result<Json<Vec<ChangeHistoryResponse>>, AppError> {
    let entries = state
        .change_history_use_cases
        .list(p.target_kind.as_deref(), p.target_id, p.limit)
        .await
        .map_err(map_err)?;
    Ok(Json(
        entries
            .into_iter()
            .map(ChangeHistoryResponse::from)
            .collect(),
    ))
}

/// 変更履歴詳細
#[utoipa::path(
    get,
    path = "/api/history/{id}",
    tag = "history",
    params(("id" = Uuid, Path, description = "変更履歴 ID")),
    responses(
        (status = 200, body = ChangeHistoryResponse),
        (status = 400, description = "リクエストパラメータが不正", body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn get_history(
    State(state): State<AppState>,
    JsonPath(id): JsonPath<Uuid>,
) -> Result<Json<ChangeHistoryResponse>, AppError> {
    let entry = state
        .change_history_use_cases
        .get(id)
        .await
        .map_err(map_err)?;
    Ok(Json(ChangeHistoryResponse::from(entry)))
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;
    use gateway_postgres::entities::change_history;
    use sea_orm::ActiveModelTrait;
    use sea_orm::ActiveValue::Set;
    use serde_json::json;
    use uuid::Uuid;

    use crate::testing::create_test_server;

    #[backend_test_macros::database_test]
    async fn get_history_returns_full_row(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db.clone()).await;
        let history_id = Uuid::from_u128(1);
        let target_id = Uuid::from_u128(2);
        change_history::ActiveModel {
            id: Set(history_id),
            target_kind: Set("note".to_string()),
            target_id: Set(target_id),
            actor_kind: Set("human".to_string()),
            actor_label: Set("sample_user".to_string()),
            op: Set("create".to_string()),
            diff_json: Set(json!({ "name": "sample_entry" })),
            summary: Set(Some("sample summary".to_string())),
            created_at: Set(chrono::DateTime::<chrono::Utc>::UNIX_EPOCH.fixed_offset()),
        }
        .insert(&db)
        .await
        .expect("insert change history");

        let response = server.get(&format!("/api/history/{history_id}")).await;
        response.assert_status(StatusCode::OK);

        assert_eq!(
            response.json::<serde_json::Value>(),
            json!({
                "id": history_id,
                "target_kind": "note",
                "target_id": target_id,
                "actor_kind": "human",
                "actor_label": "sample_user",
                "op": "create",
                "diff_json": { "name": "sample_entry" },
                "summary": "sample summary",
                "created_at": "1970-01-01T00:00:00Z",
            }),
        );
    }

    #[backend_test_macros::database_test]
    async fn get_history_returns_404_for_unknown_id(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        let response = server
            .get(&format!("/api/history/{}", uuid::Uuid::from_u128(1)))
            .await;

        assert_eq!(response.status_code(), StatusCode::NOT_FOUND);
    }
}
