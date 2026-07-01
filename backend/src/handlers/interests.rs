use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use sea_orm::ActiveModelTrait;
use sea_orm::ActiveValue::{NotSet, Set};
use sea_orm::{ColumnTrait, EntityTrait, IntoActiveModel, QueryFilter, TransactionTrait};
use uuid::Uuid;

use crate::AppState;
use crate::entities::strategy_interest;
use crate::error::{AppError, ErrorResponse};
use crate::extractors::{JsonBody, JsonPath};
use crate::models::{CreateInterestRequest, UpdateInterestRequest};
use crate::services::interests::{
    DEFAULT_ORIGIN, DEFAULT_ROLE, ensure_origin, ensure_ref_kind, ensure_role,
};
use crate::services::strategies::ensure_strategy_exists;

fn normalize_ref_id(value: &str) -> Result<String, AppError> {
    let v = value.trim().to_string();
    if v.is_empty() {
        return Err(AppError::Validation("ref_id must not be empty".into()));
    }
    Ok(v)
}

async fn find_interest_or_404(
    db: &sea_orm::DatabaseConnection,
    strategy_id: Uuid,
    ref_kind: &str,
    ref_id: &str,
) -> Result<strategy_interest::Model, AppError> {
    strategy_interest::Entity::find()
        .filter(strategy_interest::Column::StrategyId.eq(strategy_id))
        .filter(strategy_interest::Column::RefKind.eq(ref_kind))
        .filter(strategy_interest::Column::RefId.eq(ref_id))
        .one(db)
        .await?
        .ok_or_else(|| {
            AppError::NotFound(format!(
                "interest ({ref_kind}, {ref_id}) not found in strategy {strategy_id}"
            ))
        })
}

/// 戦略の関心を追加する
#[utoipa::path(
    post,
    path = "/api/strategies/{id}/interests",
    tag = "strategies",
    params(("id" = Uuid, Path, description = "戦略 ID")),
    request_body = CreateInterestRequest,
    responses(
        (status = 201, body = strategy_interest::Model),
        (status = 400, body = ErrorResponse),
        (status = 409, body = ErrorResponse),
        (status = 422, description = "リクエストボディのパースに失敗", body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn create_strategy_interest(
    State(state): State<AppState>,
    JsonPath(strategy_id): JsonPath<Uuid>,
    JsonBody(p): JsonBody<CreateInterestRequest>,
) -> Result<(StatusCode, Json<strategy_interest::Model>), AppError> {
    let ref_kind = p.ref_kind.trim().to_string();
    ensure_ref_kind(&ref_kind)?;
    let ref_id = normalize_ref_id(&p.ref_id)?;
    let role = p.role.unwrap_or_else(|| DEFAULT_ROLE.to_string());
    let origin = p.origin.unwrap_or_else(|| DEFAULT_ORIGIN.to_string());
    ensure_role(&role)?;
    ensure_origin(&origin)?;

    let txn = state.db.begin().await?;
    ensure_strategy_exists(&txn, strategy_id).await?;
    let model = strategy_interest::ActiveModel {
        strategy_id: Set(strategy_id),
        ref_kind: Set(ref_kind),
        ref_id: Set(ref_id),
        role: Set(role),
        origin: Set(origin),
        created_at: NotSet,
    };
    let created = strategy_interest::Entity::insert(model)
        .exec_with_returning(&txn)
        .await?;
    txn.commit().await?;
    Ok((StatusCode::CREATED, Json(created)))
}

/// 既存の関心を更新する (role / origin のみ)
#[utoipa::path(
    patch,
    path = "/api/strategies/{id}/interests/{ref_kind}/{ref_id}",
    tag = "strategies",
    params(
        ("id" = Uuid, Path, description = "戦略 ID"),
        ("ref_kind" = String, Path, description = "参照型 (stock / indicator / sector / theme)"),
        ("ref_id" = String, Path, description = "参照 ID"),
    ),
    request_body = UpdateInterestRequest,
    responses(
        (status = 200, body = strategy_interest::Model),
        (status = 400, body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 422, description = "リクエストボディのパースに失敗", body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn update_strategy_interest(
    State(state): State<AppState>,
    JsonPath((strategy_id, ref_kind, ref_id)): JsonPath<(Uuid, String, String)>,
    JsonBody(p): JsonBody<UpdateInterestRequest>,
) -> Result<Json<strategy_interest::Model>, AppError> {
    ensure_ref_kind(&ref_kind)?;
    let current = find_interest_or_404(&state.db, strategy_id, &ref_kind, &ref_id).await?;
    let mut active = current.clone().into_active_model();
    let mut touched = false;
    if let Some(role) = p.role {
        ensure_role(&role)?;
        active.role = Set(role);
        touched = true;
    }
    if let Some(origin) = p.origin {
        ensure_origin(&origin)?;
        active.origin = Set(origin);
        touched = true;
    }
    if !touched {
        return Err(AppError::Validation(
            "at least one of role / origin must be provided".into(),
        ));
    }
    let updated = active.update(&state.db).await?;
    Ok(Json(updated))
}

/// 関心を削除する
#[utoipa::path(
    delete,
    path = "/api/strategies/{id}/interests/{ref_kind}/{ref_id}",
    tag = "strategies",
    params(
        ("id" = Uuid, Path, description = "戦略 ID"),
        ("ref_kind" = String, Path, description = "参照型"),
        ("ref_id" = String, Path, description = "参照 ID"),
    ),
    responses(
        (status = 204),
        (status = 400, body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn delete_strategy_interest(
    State(state): State<AppState>,
    JsonPath((strategy_id, ref_kind, ref_id)): JsonPath<(Uuid, String, String)>,
) -> Result<StatusCode, AppError> {
    ensure_ref_kind(&ref_kind)?;
    let res = strategy_interest::Entity::delete_many()
        .filter(strategy_interest::Column::StrategyId.eq(strategy_id))
        .filter(strategy_interest::Column::RefKind.eq(&ref_kind))
        .filter(strategy_interest::Column::RefId.eq(&ref_id))
        .exec(&state.db)
        .await?;
    if res.rows_affected == 0 {
        return Err(AppError::NotFound(format!(
            "interest ({ref_kind}, {ref_id}) not found in strategy {strategy_id}"
        )));
    }
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;
    use sea_orm::EntityTrait;
    use serde_json::json;
    use sqlx::PgPool;

    use crate::entities::strategy_interest;
    use crate::testing::{create_test_server_with_db, insert_test_strategy};

    #[sqlx::test(migrations = false)]
    async fn create_then_list_interest_round_trips(pool: PgPool) {
        let (db, server) = create_test_server_with_db(pool).await;
        let sid = insert_test_strategy(&db, "s").await;

        let created = server
            .post(&format!("/api/strategies/{sid}/interests"))
            .json(&json!({
                "ref_kind": "stock",
                "ref_id": "7203",
            }))
            .await;
        created.assert_status(StatusCode::CREATED);

        let list = server
            .get(&format!("/api/strategies/{sid}/interests"))
            .await;
        list.assert_status_ok();
        let body: Vec<serde_json::Value> = list.json();
        let normalized: Vec<_> = body
            .into_iter()
            .map(|mut r| {
                r.as_object_mut().unwrap().remove("created_at");
                r
            })
            .collect();
        assert_eq!(
            normalized,
            vec![json!({
                "strategy_id": sid,
                "ref_kind": "stock",
                "ref_id": "7203",
                "role": "seed",
                "origin": "human",
            })],
        );
    }

    #[sqlx::test(migrations = false)]
    async fn create_with_explicit_role_origin(pool: PgPool) {
        let (db, server) = create_test_server_with_db(pool).await;
        let sid = insert_test_strategy(&db, "s").await;

        let created = server
            .post(&format!("/api/strategies/{sid}/interests"))
            .json(&json!({
                "ref_kind": "indicator",
                "ref_id": "USDJPY",
                "role": "derived",
                "origin": "llm",
            }))
            .await;
        created.assert_status(StatusCode::CREATED);
        let mut body: serde_json::Value = created.json();
        body.as_object_mut().unwrap().remove("created_at");
        assert_eq!(
            body,
            json!({
                "strategy_id": sid,
                "ref_kind": "indicator",
                "ref_id": "USDJPY",
                "role": "derived",
                "origin": "llm",
            }),
        );
    }

    #[sqlx::test(migrations = false)]
    async fn create_rejects_invalid_ref_kind_role_origin(pool: PgPool) {
        let (db, server) = create_test_server_with_db(pool).await;
        let sid = insert_test_strategy(&db, "s").await;

        for (label, body) in [
            (
                "invalid_ref_kind",
                json!({"ref_kind": "bogus", "ref_id": "x"}),
            ),
            ("empty_ref_id", json!({"ref_kind": "stock", "ref_id": ""})),
            (
                "invalid_role",
                json!({"ref_kind": "stock", "ref_id": "7203", "role": "bogus"}),
            ),
            (
                "invalid_origin",
                json!({"ref_kind": "stock", "ref_id": "7203", "origin": "bogus"}),
            ),
        ] {
            let res = server
                .post(&format!("/api/strategies/{sid}/interests"))
                .json(&body)
                .await;
            assert_eq!(
                res.status_code(),
                StatusCode::BAD_REQUEST,
                "case {label} did not return 400",
            );
        }
    }

    #[sqlx::test(migrations = false)]
    async fn create_for_unknown_strategy_returns_400(pool: PgPool) {
        let (_db, server) = create_test_server_with_db(pool).await;
        let res = server
            .post("/api/strategies/00000000-0000-0000-0000-000000000000/interests")
            .json(&json!({ "ref_kind": "stock", "ref_id": "7203" }))
            .await;
        res.assert_status(StatusCode::BAD_REQUEST);
    }

    #[sqlx::test(migrations = false)]
    async fn duplicate_create_returns_409(pool: PgPool) {
        let (db, server) = create_test_server_with_db(pool).await;
        let sid = insert_test_strategy(&db, "s").await;

        let body = json!({"ref_kind": "stock", "ref_id": "7203"});
        let first = server
            .post(&format!("/api/strategies/{sid}/interests"))
            .json(&body)
            .await;
        first.assert_status(StatusCode::CREATED);
        let dup = server
            .post(&format!("/api/strategies/{sid}/interests"))
            .json(&body)
            .await;
        dup.assert_status(StatusCode::CONFLICT);
    }

    #[sqlx::test(migrations = false)]
    async fn update_changes_role_and_origin(pool: PgPool) {
        let (db, server) = create_test_server_with_db(pool).await;
        let sid = insert_test_strategy(&db, "s").await;
        server
            .post(&format!("/api/strategies/{sid}/interests"))
            .json(&json!({"ref_kind": "stock", "ref_id": "7203"}))
            .await
            .assert_status(StatusCode::CREATED);

        let res = server
            .patch(&format!("/api/strategies/{sid}/interests/stock/7203"))
            .json(&json!({"role": "derived", "origin": "llm"}))
            .await;
        res.assert_status_ok();
        let mut body: serde_json::Value = res.json();
        body.as_object_mut().unwrap().remove("created_at");
        assert_eq!(
            body,
            json!({
                "strategy_id": sid,
                "ref_kind": "stock",
                "ref_id": "7203",
                "role": "derived",
                "origin": "llm",
            }),
        );
    }

    #[sqlx::test(migrations = false)]
    async fn update_empty_body_rejected(pool: PgPool) {
        let (db, server) = create_test_server_with_db(pool).await;
        let sid = insert_test_strategy(&db, "s").await;
        server
            .post(&format!("/api/strategies/{sid}/interests"))
            .json(&json!({"ref_kind": "stock", "ref_id": "7203"}))
            .await
            .assert_status(StatusCode::CREATED);
        let res = server
            .patch(&format!("/api/strategies/{sid}/interests/stock/7203"))
            .json(&json!({}))
            .await;
        res.assert_status(StatusCode::BAD_REQUEST);
    }

    #[sqlx::test(migrations = false)]
    async fn update_unknown_returns_404(pool: PgPool) {
        let (db, server) = create_test_server_with_db(pool).await;
        let sid = insert_test_strategy(&db, "s").await;
        let res = server
            .patch(&format!("/api/strategies/{sid}/interests/stock/9999"))
            .json(&json!({"role": "derived"}))
            .await;
        res.assert_status(StatusCode::NOT_FOUND);
    }

    #[sqlx::test(migrations = false)]
    async fn delete_existing_returns_204_then_404(pool: PgPool) {
        let (db, server) = create_test_server_with_db(pool).await;
        let sid = insert_test_strategy(&db, "s").await;
        server
            .post(&format!("/api/strategies/{sid}/interests"))
            .json(&json!({"ref_kind": "stock", "ref_id": "7203"}))
            .await
            .assert_status(StatusCode::CREATED);

        let del = server
            .delete(&format!("/api/strategies/{sid}/interests/stock/7203"))
            .await;
        del.assert_status(StatusCode::NO_CONTENT);

        let rows = strategy_interest::Entity::find()
            .all(&db)
            .await
            .expect("rows");
        assert!(rows.is_empty());

        let again = server
            .delete(&format!("/api/strategies/{sid}/interests/stock/7203"))
            .await;
        again.assert_status(StatusCode::NOT_FOUND);
    }

    #[sqlx::test(migrations = false)]
    async fn interests_are_isolated_per_strategy(pool: PgPool) {
        let (db, server) = create_test_server_with_db(pool).await;
        let a = insert_test_strategy(&db, "a").await;
        let b = insert_test_strategy(&db, "b").await;
        server
            .post(&format!("/api/strategies/{a}/interests"))
            .json(&json!({"ref_kind": "stock", "ref_id": "7203"}))
            .await
            .assert_status(StatusCode::CREATED);
        server
            .post(&format!("/api/strategies/{b}/interests"))
            .json(&json!({"ref_kind": "stock", "ref_id": "9984"}))
            .await
            .assert_status(StatusCode::CREATED);

        let list_a = server.get(&format!("/api/strategies/{a}/interests")).await;
        let list_b = server.get(&format!("/api/strategies/{b}/interests")).await;

        let normalize = |list: serde_json::Value| -> Vec<serde_json::Value> {
            list.as_array()
                .unwrap()
                .iter()
                .map(|r| {
                    let mut r = r.clone();
                    r.as_object_mut().unwrap().remove("created_at");
                    r
                })
                .collect()
        };
        assert_eq!(
            (
                normalize(list_a.json::<serde_json::Value>()),
                normalize(list_b.json::<serde_json::Value>()),
            ),
            (
                vec![json!({
                    "strategy_id": a,
                    "ref_kind": "stock",
                    "ref_id": "7203",
                    "role": "seed",
                    "origin": "human",
                })],
                vec![json!({
                    "strategy_id": b,
                    "ref_kind": "stock",
                    "ref_id": "9984",
                    "role": "seed",
                    "origin": "human",
                })],
            ),
        );

        // 削除も自分の戦略の interest しか消えない
        server
            .delete(&format!("/api/strategies/{a}/interests/stock/9984"))
            .await
            .assert_status(StatusCode::NOT_FOUND);
    }
}
