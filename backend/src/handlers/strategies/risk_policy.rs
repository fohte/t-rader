use axum::Json;
use axum::extract::State;
use uuid::Uuid;

use crate::AppState;
use crate::error::{AppError, ErrorResponse};
use crate::extractors::{JsonBody, JsonPath};
use crate::models::{
    PutStrategyRiskPolicyRequest, StrategyRiskPolicyData, StrategyRiskPolicyResponse,
    parse_risk_policy, serialize_risk_policy, validate_ratio,
};
use crate::services::change_history::Actor;
use crate::services::strategy_config;

use super::find_strategy_or_404;

/// 戦略の銘柄集中度上限 (`max_position_ratio`) を取得
#[utoipa::path(
    get,
    path = "/api/strategies/{id}/risk-policy",
    tag = "strategies",
    params(("id" = Uuid, Path, description = "戦略 ID")),
    responses(
        (status = 200, body = StrategyRiskPolicyResponse),
        (status = 400, description = "リクエストパラメータが不正", body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn get_risk_policy(
    State(state): State<AppState>,
    JsonPath(id): JsonPath<Uuid>,
) -> Result<Json<StrategyRiskPolicyResponse>, AppError> {
    let row = find_strategy_or_404(&state.db, id).await?;
    let data = parse_risk_policy::<StrategyRiskPolicyData>(row.risk_policy)?;
    Ok(Json(data.into()))
}

/// 戦略の銘柄集中度上限 (`max_position_ratio`) を更新。`null` で上限を解除する
#[utoipa::path(
    put,
    path = "/api/strategies/{id}/risk-policy",
    tag = "strategies",
    params(("id" = Uuid, Path, description = "戦略 ID")),
    request_body = PutStrategyRiskPolicyRequest,
    responses(
        (status = 200, body = StrategyRiskPolicyResponse),
        (status = 400, body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 415, description = "Content-Type ヘッダが application/json ではない", body = ErrorResponse),
        (status = 422, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn put_risk_policy(
    State(state): State<AppState>,
    JsonPath(id): JsonPath<Uuid>,
    JsonBody(payload): JsonBody<PutStrategyRiskPolicyRequest>,
) -> Result<Json<StrategyRiskPolicyResponse>, AppError> {
    let current = find_strategy_or_404(&state.db, id).await?;
    validate_ratio(payload.max_position_ratio)?;
    let data = StrategyRiskPolicyData {
        schema_version: crate::models::risk_policy::RISK_POLICY_SCHEMA_VERSION,
        max_position_ratio: payload.max_position_ratio,
    };
    let value = serialize_risk_policy(&data)?;
    let updated =
        strategy_config::save_risk_policy(&state.db, Actor::Human, current, value).await?;
    let data = parse_risk_policy::<StrategyRiskPolicyData>(updated.risk_policy)?;
    Ok(Json(data.into()))
}

#[cfg(test)]
mod tests {
    use sea_orm::ActiveModelTrait;
    use sea_orm::ActiveValue::Set;
    use sqlx::PgPool;

    use crate::testing::{create_strategy, create_test_server, create_test_server_with_db};

    #[sqlx::test(migrations = false)]
    async fn get_returns_null_when_unset(pool: PgPool) {
        let server = create_test_server(pool).await;
        let id = create_strategy(&server, "s").await;

        let res = server
            .get(&format!("/api/strategies/{id}/risk-policy"))
            .await;
        res.assert_status_ok();
        assert_eq!(
            res.json::<serde_json::Value>(),
            serde_json::json!({ "max_position_ratio": null }),
        );
    }

    #[sqlx::test(migrations = false)]
    async fn put_then_get_round_trips(pool: PgPool) {
        let server = create_test_server(pool).await;
        let id = create_strategy(&server, "s").await;

        let expected = serde_json::json!({ "max_position_ratio": 0.2 });
        let put = server
            .put(&format!("/api/strategies/{id}/risk-policy"))
            .json(&serde_json::json!({ "max_position_ratio": 0.2 }))
            .await;
        put.assert_status_ok();
        assert_eq!(put.json::<serde_json::Value>(), expected);

        let get = server
            .get(&format!("/api/strategies/{id}/risk-policy"))
            .await;
        get.assert_status_ok();
        assert_eq!(get.json::<serde_json::Value>(), expected);
    }

    #[sqlx::test(migrations = false)]
    async fn put_null_clears_limit(pool: PgPool) {
        let server = create_test_server(pool).await;
        let id = create_strategy(&server, "s").await;

        server
            .put(&format!("/api/strategies/{id}/risk-policy"))
            .json(&serde_json::json!({ "max_position_ratio": 0.2 }))
            .await
            .assert_status_ok();
        let cleared = server
            .put(&format!("/api/strategies/{id}/risk-policy"))
            .json(&serde_json::json!({ "max_position_ratio": null }))
            .await;
        cleared.assert_status_ok();
        assert_eq!(
            cleared.json::<serde_json::Value>(),
            serde_json::json!({ "max_position_ratio": null }),
        );

        let get = server
            .get(&format!("/api/strategies/{id}/risk-policy"))
            .await;
        assert_eq!(
            get.json::<serde_json::Value>(),
            serde_json::json!({ "max_position_ratio": null }),
        );
    }

    #[sqlx::test(migrations = false)]
    async fn put_400_for_out_of_range_ratio(pool: PgPool) {
        let server = create_test_server(pool).await;
        let id = create_strategy(&server, "s").await;

        for invalid in [serde_json::json!(0), serde_json::json!(1.5)] {
            let res = server
                .put(&format!("/api/strategies/{id}/risk-policy"))
                .json(&serde_json::json!({ "max_position_ratio": invalid }))
                .await;
            res.assert_status(axum::http::StatusCode::BAD_REQUEST);
        }
    }

    #[sqlx::test(migrations = false)]
    async fn get_404_for_unknown_strategy(pool: PgPool) {
        let server = create_test_server(pool).await;
        let res = server
            .get("/api/strategies/00000000-0000-0000-0000-000000000000/risk-policy")
            .await;
        res.assert_status(axum::http::StatusCode::NOT_FOUND);
    }

    #[sqlx::test(migrations = false)]
    async fn put_404_for_unknown_strategy(pool: PgPool) {
        let server = create_test_server(pool).await;
        let res = server
            .put("/api/strategies/00000000-0000-0000-0000-000000000000/risk-policy")
            .json(&serde_json::json!({ "max_position_ratio": 0.2 }))
            .await;
        res.assert_status(axum::http::StatusCode::NOT_FOUND);
    }

    #[sqlx::test(migrations = false)]
    async fn get_tolerates_unknown_fields_in_stored_json(pool: PgPool) {
        let (db, server) = create_test_server_with_db(pool).await;
        let id = create_strategy(&server, "s").await;
        let id = uuid::Uuid::parse_str(&id).expect("parse id");

        let current = crate::services::strategy_config::find_or_404(&db, id)
            .await
            .expect("find strategy");
        let mut active = sea_orm::IntoActiveModel::into_active_model(current);
        active.risk_policy = Set(serde_json::json!({
            "schema_version": 1,
            "max_position_ratio": 0.15,
            "future_field": "x",
        }));
        active.update(&db).await.expect("seed risk_policy directly");

        let res = server
            .get(&format!("/api/strategies/{id}/risk-policy"))
            .await;
        res.assert_status_ok();
        assert_eq!(
            res.json::<serde_json::Value>(),
            serde_json::json!({ "max_position_ratio": 0.15 }),
        );
    }
}
