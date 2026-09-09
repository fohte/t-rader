use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use sea_orm::ActiveModelTrait;
use sea_orm::ActiveValue::{NotSet, Set};
use sea_orm::{
    ColumnTrait, EntityTrait, IntoActiveModel, QueryFilter, QueryOrder, TransactionTrait,
};
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

/// 関心が見つからなかった場合の 404 メッセージ。global (`strategy_id: None`) と
/// 戦略スコープ (`strategy_id: Some`) とで文言を分ける。
fn interest_not_found_message(strategy_id: Option<Uuid>, ref_kind: &str, ref_id: &str) -> String {
    match strategy_id {
        Some(id) => format!("interest ({ref_kind}, {ref_id}) not found in strategy {id}"),
        None => format!("interest ({ref_kind}, {ref_id}) not found in global interests"),
    }
}

async fn find_interest_or_404(
    db: &sea_orm::DatabaseConnection,
    strategy_id: Option<Uuid>,
    ref_kind: &str,
    ref_id: &str,
) -> Result<strategy_interest::Model, AppError> {
    let mut query = strategy_interest::Entity::find()
        .filter(strategy_interest::Column::RefKind.eq(ref_kind))
        .filter(strategy_interest::Column::RefId.eq(ref_id));
    query = match strategy_id {
        Some(id) => query.filter(strategy_interest::Column::StrategyId.eq(id)),
        None => query.filter(strategy_interest::Column::StrategyId.is_null()),
    };
    query.one(db).await?.ok_or_else(|| {
        AppError::NotFound(interest_not_found_message(strategy_id, ref_kind, ref_id))
    })
}

/// role / origin を更新する共通処理。戦略スコープ / global スコープ両方から呼ばれる。
async fn update_interest_inner(
    db: &sea_orm::DatabaseConnection,
    strategy_id: Option<Uuid>,
    ref_kind: &str,
    ref_id: &str,
    p: UpdateInterestRequest,
) -> Result<strategy_interest::Model, AppError> {
    ensure_ref_kind(ref_kind)?;
    let current = find_interest_or_404(db, strategy_id, ref_kind, ref_id).await?;
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
    active.update(db).await.map_err(AppError::from)
}

/// 削除する共通処理。戦略スコープ / global スコープ両方から呼ばれる。
async fn delete_interest_inner(
    db: &sea_orm::DatabaseConnection,
    strategy_id: Option<Uuid>,
    ref_kind: &str,
    ref_id: &str,
) -> Result<(), AppError> {
    ensure_ref_kind(ref_kind)?;
    let mut query = strategy_interest::Entity::delete_many()
        .filter(strategy_interest::Column::RefKind.eq(ref_kind))
        .filter(strategy_interest::Column::RefId.eq(ref_id));
    query = match strategy_id {
        Some(id) => query.filter(strategy_interest::Column::StrategyId.eq(id)),
        None => query.filter(strategy_interest::Column::StrategyId.is_null()),
    };
    let res = query.exec(db).await?;
    if res.rows_affected == 0 {
        return Err(AppError::NotFound(interest_not_found_message(
            strategy_id,
            ref_kind,
            ref_id,
        )));
    }
    Ok(())
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
        (status = 415, description = "Content-Type ヘッダが application/json ではない", body = ErrorResponse),
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
        id: NotSet,
        strategy_id: Set(Some(strategy_id)),
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
        (status = 415, description = "Content-Type ヘッダが application/json ではない", body = ErrorResponse),
        (status = 422, description = "リクエストボディのパースに失敗", body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn update_strategy_interest(
    State(state): State<AppState>,
    JsonPath((strategy_id, ref_kind, ref_id)): JsonPath<(Uuid, String, String)>,
    JsonBody(p): JsonBody<UpdateInterestRequest>,
) -> Result<Json<strategy_interest::Model>, AppError> {
    let updated =
        update_interest_inner(&state.db, Some(strategy_id), &ref_kind, &ref_id, p).await?;
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
    delete_interest_inner(&state.db, Some(strategy_id), &ref_kind, &ref_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// どの戦略にも属さない関心 (口座全体として追う日経平均・業種・テーマなど) の一覧を取得する
#[utoipa::path(
    get,
    path = "/api/interests",
    tag = "strategies",
    responses(
        (status = 200, body = Vec<strategy_interest::Model>),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn list_global_interests(
    State(state): State<AppState>,
) -> Result<Json<Vec<strategy_interest::Model>>, AppError> {
    let items = strategy_interest::Entity::find()
        .filter(strategy_interest::Column::StrategyId.is_null())
        .order_by_asc(strategy_interest::Column::Role)
        .order_by_asc(strategy_interest::Column::CreatedAt)
        .all(&state.db)
        .await?;
    Ok(Json(items))
}

/// どの戦略にも属さない関心を追加する
#[utoipa::path(
    post,
    path = "/api/interests",
    tag = "strategies",
    request_body = CreateInterestRequest,
    responses(
        (status = 201, body = strategy_interest::Model),
        (status = 400, body = ErrorResponse),
        (status = 409, body = ErrorResponse),
        (status = 415, description = "Content-Type ヘッダが application/json ではない", body = ErrorResponse),
        (status = 422, description = "リクエストボディのパースに失敗", body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn create_global_interest(
    State(state): State<AppState>,
    JsonBody(p): JsonBody<CreateInterestRequest>,
) -> Result<(StatusCode, Json<strategy_interest::Model>), AppError> {
    let ref_kind = p.ref_kind.trim().to_string();
    ensure_ref_kind(&ref_kind)?;
    let ref_id = normalize_ref_id(&p.ref_id)?;
    let role = p.role.unwrap_or_else(|| DEFAULT_ROLE.to_string());
    let origin = p.origin.unwrap_or_else(|| DEFAULT_ORIGIN.to_string());
    ensure_role(&role)?;
    ensure_origin(&origin)?;

    let model = strategy_interest::ActiveModel {
        id: NotSet,
        strategy_id: Set(None),
        ref_kind: Set(ref_kind),
        ref_id: Set(ref_id),
        role: Set(role),
        origin: Set(origin),
        created_at: NotSet,
    };
    let created = strategy_interest::Entity::insert(model)
        .exec_with_returning(&state.db)
        .await?;
    Ok((StatusCode::CREATED, Json(created)))
}

/// どの戦略にも属さない関心を更新する (role / origin のみ)
#[utoipa::path(
    patch,
    path = "/api/interests/{ref_kind}/{ref_id}",
    tag = "strategies",
    params(
        ("ref_kind" = String, Path, description = "参照型 (stock / indicator / sector / theme)"),
        ("ref_id" = String, Path, description = "参照 ID"),
    ),
    request_body = UpdateInterestRequest,
    responses(
        (status = 200, body = strategy_interest::Model),
        (status = 400, body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 415, description = "Content-Type ヘッダが application/json ではない", body = ErrorResponse),
        (status = 422, description = "リクエストボディのパースに失敗", body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn update_global_interest(
    State(state): State<AppState>,
    JsonPath((ref_kind, ref_id)): JsonPath<(String, String)>,
    JsonBody(p): JsonBody<UpdateInterestRequest>,
) -> Result<Json<strategy_interest::Model>, AppError> {
    let updated = update_interest_inner(&state.db, None, &ref_kind, &ref_id, p).await?;
    Ok(Json(updated))
}

/// どの戦略にも属さない関心を削除する
#[utoipa::path(
    delete,
    path = "/api/interests/{ref_kind}/{ref_id}",
    tag = "strategies",
    params(
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
pub async fn delete_global_interest(
    State(state): State<AppState>,
    JsonPath((ref_kind, ref_id)): JsonPath<(String, String)>,
) -> Result<StatusCode, AppError> {
    delete_interest_inner(&state.db, None, &ref_kind, &ref_id).await?;
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
                let obj = r.as_object_mut().unwrap();
                obj.remove("created_at");
                obj.remove("id");
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
        let obj = body.as_object_mut().unwrap();
        obj.remove("created_at");
        obj.remove("id");
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
        let obj = body.as_object_mut().unwrap();
        obj.remove("created_at");
        obj.remove("id");
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
                    let obj = r.as_object_mut().unwrap();
                    obj.remove("created_at");
                    obj.remove("id");
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

    #[sqlx::test(migrations = false)]
    async fn create_then_list_global_interest_round_trips(pool: PgPool) {
        let (_db, server) = create_test_server_with_db(pool).await;

        let created = server
            .post("/api/interests")
            .json(&json!({
                "ref_kind": "indicator",
                "ref_id": "N225",
            }))
            .await;
        created.assert_status(StatusCode::CREATED);

        let list = server.get("/api/interests").await;
        list.assert_status_ok();
        let body: Vec<serde_json::Value> = list.json();
        let normalized: Vec<_> = body
            .into_iter()
            .map(|mut r| {
                let obj = r.as_object_mut().unwrap();
                obj.remove("created_at");
                obj.remove("id");
                r
            })
            .collect();
        assert_eq!(
            normalized,
            vec![json!({
                "strategy_id": null,
                "ref_kind": "indicator",
                "ref_id": "N225",
                "role": "seed",
                "origin": "human",
            })],
        );
    }

    #[sqlx::test(migrations = false)]
    async fn duplicate_global_create_returns_409(pool: PgPool) {
        let (_db, server) = create_test_server_with_db(pool).await;

        let body = json!({"ref_kind": "indicator", "ref_id": "N225"});
        let first = server.post("/api/interests").json(&body).await;
        first.assert_status(StatusCode::CREATED);
        let dup = server.post("/api/interests").json(&body).await;
        dup.assert_status(StatusCode::CONFLICT);
    }

    #[sqlx::test(migrations = false)]
    async fn update_global_interest_changes_role_and_origin(pool: PgPool) {
        let (_db, server) = create_test_server_with_db(pool).await;
        server
            .post("/api/interests")
            .json(&json!({"ref_kind": "indicator", "ref_id": "N225"}))
            .await
            .assert_status(StatusCode::CREATED);

        let res = server
            .patch("/api/interests/indicator/N225")
            .json(&json!({"role": "derived", "origin": "llm"}))
            .await;
        res.assert_status_ok();
        let mut body: serde_json::Value = res.json();
        let obj = body.as_object_mut().unwrap();
        obj.remove("created_at");
        obj.remove("id");
        assert_eq!(
            body,
            json!({
                "strategy_id": null,
                "ref_kind": "indicator",
                "ref_id": "N225",
                "role": "derived",
                "origin": "llm",
            }),
        );
    }

    #[sqlx::test(migrations = false)]
    async fn delete_global_interest_returns_204_then_404(pool: PgPool) {
        let (db, server) = create_test_server_with_db(pool).await;
        server
            .post("/api/interests")
            .json(&json!({"ref_kind": "indicator", "ref_id": "N225"}))
            .await
            .assert_status(StatusCode::CREATED);

        let del = server.delete("/api/interests/indicator/N225").await;
        del.assert_status(StatusCode::NO_CONTENT);

        let rows = strategy_interest::Entity::find()
            .all(&db)
            .await
            .expect("rows");
        assert!(rows.is_empty());

        let again = server.delete("/api/interests/indicator/N225").await;
        again.assert_status(StatusCode::NOT_FOUND);
    }

    /// 同じ (ref_kind, ref_id) が strategy スコープと global スコープで共存できることの
    /// 回帰テスト (部分ユニークインデックスが正しく機能していないと片方が 409 になる)
    #[sqlx::test(migrations = false)]
    async fn strategy_and_global_interest_coexist_for_same_ref(pool: PgPool) {
        let (_db, server) = create_test_server_with_db(pool).await;
        let sid = insert_test_strategy(&_db, "s").await;

        let scoped = server
            .post(&format!("/api/strategies/{sid}/interests"))
            .json(&json!({"ref_kind": "indicator", "ref_id": "N225"}))
            .await;
        scoped.assert_status(StatusCode::CREATED);

        let global = server
            .post("/api/interests")
            .json(&json!({"ref_kind": "indicator", "ref_id": "N225"}))
            .await;
        global.assert_status(StatusCode::CREATED);

        let scoped_list = server
            .get(&format!("/api/strategies/{sid}/interests"))
            .await;
        let global_list = server.get("/api/interests").await;

        let normalize = |v: serde_json::Value| -> Vec<serde_json::Value> {
            v.as_array()
                .unwrap()
                .iter()
                .map(|r| {
                    let mut r = r.clone();
                    let obj = r.as_object_mut().unwrap();
                    obj.remove("created_at");
                    obj.remove("id");
                    r
                })
                .collect()
        };
        assert_eq!(
            (
                normalize(scoped_list.json::<serde_json::Value>()),
                normalize(global_list.json::<serde_json::Value>()),
            ),
            (
                vec![json!({
                    "strategy_id": sid,
                    "ref_kind": "indicator",
                    "ref_id": "N225",
                    "role": "seed",
                    "origin": "human",
                })],
                vec![json!({
                    "strategy_id": null,
                    "ref_kind": "indicator",
                    "ref_id": "N225",
                    "role": "seed",
                    "origin": "human",
                })],
            ),
        );
    }
}
