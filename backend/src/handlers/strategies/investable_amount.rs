use axum::Json;
use axum::extract::State;
use chrono::Utc;
use uuid::Uuid;

use crate::AppState;
use crate::error::{AppError, ErrorResponse};
use crate::extractors::{JsonBody, JsonPath};
use crate::models::{InvestableAmountResponse, PutInvestableAmountRequest};
use crate::services::investable_amount;

use super::find_strategy_or_404;

fn to_response(
    current: Option<crate::entities::strategy_investable_amount::Model>,
) -> InvestableAmountResponse {
    InvestableAmountResponse {
        amount_jpy: current.as_ref().map(|m| m.amount_jpy),
        effective_at: current.map(|m| m.effective_at),
    }
}

/// 戦略の投資可能額 (`effective_at` が現在時刻以下の最新行) を取得。
/// history が無い戦略では `amount_jpy` / `effective_at` ともに null を返す。
#[utoipa::path(
    get,
    path = "/api/strategies/{id}/investable-amount",
    tag = "strategies",
    params(("id" = Uuid, Path, description = "戦略 ID")),
    responses(
        (status = 200, body = InvestableAmountResponse),
        (status = 400, description = "リクエストパラメータが不正", body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn get_investable_amount(
    State(state): State<AppState>,
    JsonPath(id): JsonPath<Uuid>,
) -> Result<Json<InvestableAmountResponse>, AppError> {
    find_strategy_or_404(&state.db, id).await?;
    let current = investable_amount::find_current(&state.db, id).await?;
    Ok(Json(to_response(current)))
}

/// 戦略の投資可能額を新しい history 行として記録する。既存行は上書きしない。
#[utoipa::path(
    put,
    path = "/api/strategies/{id}/investable-amount",
    tag = "strategies",
    params(("id" = Uuid, Path, description = "戦略 ID")),
    request_body = PutInvestableAmountRequest,
    responses(
        (status = 200, body = InvestableAmountResponse),
        (status = 400, body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 415, description = "Content-Type ヘッダが application/json ではない", body = ErrorResponse),
        (status = 422, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn put_investable_amount(
    State(state): State<AppState>,
    JsonPath(id): JsonPath<Uuid>,
    JsonBody(payload): JsonBody<PutInvestableAmountRequest>,
) -> Result<Json<InvestableAmountResponse>, AppError> {
    find_strategy_or_404(&state.db, id).await?;
    let effective_at = payload
        .effective_at
        .unwrap_or_else(|| Utc::now().fixed_offset());
    let created = investable_amount::set(&state.db, id, payload.amount_jpy, effective_at).await?;
    Ok(Json(to_response(Some(created))))
}

#[cfg(test)]
mod tests {
    use sqlx::PgPool;

    use crate::testing::{create_strategy, create_test_server};

    #[sqlx::test(migrations = false)]
    async fn get_returns_null_when_unset(pool: PgPool) {
        let server = create_test_server(pool).await;
        let id = create_strategy(&server, "s").await;

        let res = server
            .get(&format!("/api/strategies/{id}/investable-amount"))
            .await;
        res.assert_status_ok();
        assert_eq!(
            res.json::<serde_json::Value>(),
            serde_json::json!({ "amount_jpy": null, "effective_at": null }),
        );
    }

    #[sqlx::test(migrations = false)]
    async fn put_then_get_round_trips(pool: PgPool) {
        let server = create_test_server(pool).await;
        let id = create_strategy(&server, "s").await;

        let expected = serde_json::json!({
            "amount_jpy": 1000000,
            "effective_at": "2020-01-01T00:00:00Z",
        });
        let put = server
            .put(&format!("/api/strategies/{id}/investable-amount"))
            .json(&serde_json::json!({
                "amount_jpy": 1000000,
                "effective_at": "2020-01-01T00:00:00Z",
            }))
            .await;
        put.assert_status_ok();
        assert_eq!(put.json::<serde_json::Value>(), expected);

        let get = server
            .get(&format!("/api/strategies/{id}/investable-amount"))
            .await;
        get.assert_status_ok();
        assert_eq!(get.json::<serde_json::Value>(), expected);
    }

    #[sqlx::test(migrations = false)]
    async fn put_without_effective_at_defaults_to_now(pool: PgPool) {
        let server = create_test_server(pool).await;
        let id = create_strategy(&server, "s").await;
        let before = chrono::Utc::now();

        let put = server
            .put(&format!("/api/strategies/{id}/investable-amount"))
            .json(&serde_json::json!({ "amount_jpy": 500000 }))
            .await;
        put.assert_status_ok();
        let body: serde_json::Value = put.json();
        let effective_at: chrono::DateTime<chrono::Utc> = body["effective_at"]
            .as_str()
            .expect("effective_at is a string")
            .parse()
            .expect("valid RFC3339 timestamp");

        assert_eq!(body["amount_jpy"], serde_json::json!(500000));
        assert!(effective_at >= before);
    }

    #[sqlx::test(migrations = false)]
    async fn put_keeps_previous_history_row(pool: PgPool) {
        let server = create_test_server(pool).await;
        let id = create_strategy(&server, "s").await;

        server
            .put(&format!("/api/strategies/{id}/investable-amount"))
            .json(&serde_json::json!({
                "amount_jpy": 1000000,
                "effective_at": "2020-01-01T00:00:00Z",
            }))
            .await
            .assert_status_ok();
        server
            .put(&format!("/api/strategies/{id}/investable-amount"))
            .json(&serde_json::json!({
                "amount_jpy": 2000000,
                "effective_at": "2020-06-01T00:00:00Z",
            }))
            .await
            .assert_status_ok();

        let get = server
            .get(&format!("/api/strategies/{id}/investable-amount"))
            .await;
        get.assert_status_ok();
        assert_eq!(
            get.json::<serde_json::Value>(),
            serde_json::json!({
                "amount_jpy": 2000000,
                "effective_at": "2020-06-01T00:00:00Z",
            }),
        );
    }

    #[sqlx::test(migrations = false)]
    async fn get_404_for_unknown_strategy(pool: PgPool) {
        let server = create_test_server(pool).await;
        let res = server
            .get("/api/strategies/00000000-0000-0000-0000-000000000000/investable-amount")
            .await;
        res.assert_status(axum::http::StatusCode::NOT_FOUND);
    }

    #[sqlx::test(migrations = false)]
    async fn put_404_for_unknown_strategy(pool: PgPool) {
        let server = create_test_server(pool).await;
        let res = server
            .put("/api/strategies/00000000-0000-0000-0000-000000000000/investable-amount")
            .json(&serde_json::json!({ "amount_jpy": 1000000 }))
            .await;
        res.assert_status(axum::http::StatusCode::NOT_FOUND);
    }
}
