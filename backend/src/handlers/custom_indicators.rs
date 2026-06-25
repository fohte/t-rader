use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use sea_orm::ActiveModelTrait;
use sea_orm::ActiveValue::{NotSet, Set};
use sea_orm::{ColumnTrait, EntityTrait, IntoActiveModel, QueryFilter, QueryOrder};
use uuid::Uuid;

use crate::AppState;
use crate::entities::custom_indicator;
use crate::error::{AppError, ErrorResponse};
use crate::extractors::{JsonBody, JsonPath};
use crate::models::{CreateCustomIndicatorRequest, UpdateCustomIndicatorRequest};
use crate::services::custom_indicators::{SCOPE_GLOBAL, SCOPE_STRATEGY};
use crate::services::strategies::ensure_strategy_exists;

fn validate_name(value: &str) -> Result<String, AppError> {
    let trimmed = value.trim().to_string();
    if trimmed.is_empty() {
        return Err(AppError::Validation("name must not be empty".into()));
    }
    Ok(trimmed)
}

fn ensure_json_object(field: &str, value: &serde_json::Value) -> Result<(), AppError> {
    if value.is_object() {
        Ok(())
    } else {
        Err(AppError::Validation(format!(
            "{field} must be a JSON object"
        )))
    }
}

async fn find_indicator_or_404(
    db: &sea_orm::DatabaseConnection,
    indicator_id: Uuid,
) -> Result<custom_indicator::Model, AppError> {
    custom_indicator::Entity::find_by_id(indicator_id)
        .one(db)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("indicator {indicator_id} not found")))
}

async fn find_strategy_scoped_or_404(
    db: &sea_orm::DatabaseConnection,
    strategy_id: Uuid,
    indicator_id: Uuid,
) -> Result<custom_indicator::Model, AppError> {
    let model = find_indicator_or_404(db, indicator_id).await?;
    if model.scope != SCOPE_STRATEGY || model.strategy_id != Some(strategy_id) {
        return Err(AppError::NotFound(format!(
            "indicator {indicator_id} not found"
        )));
    }
    Ok(model)
}

/// グローバル indicator 一覧
#[utoipa::path(
    get,
    path = "/api/indicators",
    tag = "custom_indicators",
    responses(
        (status = 200, body = Vec<custom_indicator::Model>),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn list_global_indicators(
    State(state): State<AppState>,
) -> Result<Json<Vec<custom_indicator::Model>>, AppError> {
    let items = custom_indicator::Entity::find()
        .filter(custom_indicator::Column::Scope.eq(SCOPE_GLOBAL))
        .order_by_asc(custom_indicator::Column::Name)
        .all(&state.db)
        .await?;
    Ok(Json(items))
}

/// 戦略 scope indicator 一覧
#[utoipa::path(
    get,
    path = "/api/strategies/{id}/indicators",
    tag = "custom_indicators",
    params(("id" = Uuid, Path, description = "戦略 ID")),
    responses(
        (status = 200, body = Vec<custom_indicator::Model>),
        (status = 404, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn list_strategy_indicators(
    State(state): State<AppState>,
    JsonPath(strategy_id): JsonPath<Uuid>,
) -> Result<Json<Vec<custom_indicator::Model>>, AppError> {
    ensure_strategy_exists(&state.db, strategy_id).await?;
    let items = custom_indicator::Entity::find()
        .filter(custom_indicator::Column::Scope.eq(SCOPE_STRATEGY))
        .filter(custom_indicator::Column::StrategyId.eq(strategy_id))
        .order_by_asc(custom_indicator::Column::Name)
        .all(&state.db)
        .await?;
    Ok(Json(items))
}

/// indicator 詳細
#[utoipa::path(
    get,
    path = "/api/indicators/{indicator_id}",
    tag = "custom_indicators",
    params(("indicator_id" = Uuid, Path, description = "indicator ID")),
    responses(
        (status = 200, body = custom_indicator::Model),
        (status = 404, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn get_indicator(
    State(state): State<AppState>,
    JsonPath(indicator_id): JsonPath<Uuid>,
) -> Result<Json<custom_indicator::Model>, AppError> {
    Ok(Json(find_indicator_or_404(&state.db, indicator_id).await?))
}

/// グローバル indicator 作成
#[utoipa::path(
    post,
    path = "/api/indicators",
    tag = "custom_indicators",
    request_body = CreateCustomIndicatorRequest,
    responses(
        (status = 201, body = custom_indicator::Model),
        (status = 400, body = ErrorResponse),
        (status = 409, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn create_global_indicator(
    State(state): State<AppState>,
    JsonBody(payload): JsonBody<CreateCustomIndicatorRequest>,
) -> Result<(StatusCode, Json<custom_indicator::Model>), AppError> {
    let model = insert_indicator(&state.db, payload, None).await?;
    Ok((StatusCode::CREATED, Json(model)))
}

/// 戦略 scope indicator 作成
#[utoipa::path(
    post,
    path = "/api/strategies/{id}/indicators",
    tag = "custom_indicators",
    params(("id" = Uuid, Path, description = "戦略 ID")),
    request_body = CreateCustomIndicatorRequest,
    responses(
        (status = 201, body = custom_indicator::Model),
        (status = 400, body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 409, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn create_strategy_indicator(
    State(state): State<AppState>,
    JsonPath(strategy_id): JsonPath<Uuid>,
    JsonBody(payload): JsonBody<CreateCustomIndicatorRequest>,
) -> Result<(StatusCode, Json<custom_indicator::Model>), AppError> {
    ensure_strategy_exists(&state.db, strategy_id).await?;
    let model = insert_indicator(&state.db, payload, Some(strategy_id)).await?;
    Ok((StatusCode::CREATED, Json(model)))
}

async fn insert_indicator(
    db: &sea_orm::DatabaseConnection,
    payload: CreateCustomIndicatorRequest,
    strategy_id: Option<Uuid>,
) -> Result<custom_indicator::Model, AppError> {
    let name = validate_name(&payload.name)?;
    ensure_json_object("input_schema", &payload.input_schema)?;
    ensure_json_object("output_schema", &payload.output_schema)?;

    let scope = if strategy_id.is_some() {
        SCOPE_STRATEGY
    } else {
        SCOPE_GLOBAL
    };

    let active = custom_indicator::ActiveModel {
        indicator_id: Set(Uuid::new_v4()),
        name: Set(name),
        scope: Set(scope.to_string()),
        strategy_id: Set(strategy_id),
        code: Set(payload.code),
        input_schema: Set(payload.input_schema),
        output_schema: Set(payload.output_schema),
        description: Set(payload.description),
        created_at: NotSet,
        updated_at: NotSet,
    };
    let created = custom_indicator::Entity::insert(active)
        .exec_with_returning(db)
        .await?;
    Ok(created)
}

/// indicator 更新
#[utoipa::path(
    put,
    path = "/api/indicators/{indicator_id}",
    tag = "custom_indicators",
    params(("indicator_id" = Uuid, Path, description = "indicator ID")),
    request_body = UpdateCustomIndicatorRequest,
    responses(
        (status = 200, body = custom_indicator::Model),
        (status = 400, body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 409, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn update_indicator(
    State(state): State<AppState>,
    JsonPath(indicator_id): JsonPath<Uuid>,
    JsonBody(payload): JsonBody<UpdateCustomIndicatorRequest>,
) -> Result<Json<custom_indicator::Model>, AppError> {
    let current = find_indicator_or_404(&state.db, indicator_id).await?;
    let mut active = current.into_active_model();

    if let Some(name) = payload.name {
        active.name = Set(validate_name(&name)?);
    }
    if let Some(code) = payload.code {
        active.code = Set(code);
    }
    if let Some(input_schema) = payload.input_schema {
        ensure_json_object("input_schema", &input_schema)?;
        active.input_schema = Set(input_schema);
    }
    if let Some(output_schema) = payload.output_schema {
        ensure_json_object("output_schema", &output_schema)?;
        active.output_schema = Set(output_schema);
    }
    if let Some(description) = payload.description {
        active.description = Set(Some(description));
    }
    active.updated_at = Set(chrono::Utc::now().fixed_offset());

    let updated = active.update(&state.db).await?;
    Ok(Json(updated))
}

/// indicator 削除
#[utoipa::path(
    delete,
    path = "/api/indicators/{indicator_id}",
    tag = "custom_indicators",
    params(("indicator_id" = Uuid, Path, description = "indicator ID")),
    responses(
        (status = 204),
        (status = 404, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn delete_indicator(
    State(state): State<AppState>,
    JsonPath(indicator_id): JsonPath<Uuid>,
) -> Result<StatusCode, AppError> {
    let res = custom_indicator::Entity::delete_by_id(indicator_id)
        .exec(&state.db)
        .await?;
    if res.rows_affected == 0 {
        return Err(AppError::NotFound(format!(
            "indicator {indicator_id} not found"
        )));
    }
    Ok(StatusCode::NO_CONTENT)
}

/// 戦略 scope の指定 indicator 詳細 (境界外は 404)
#[utoipa::path(
    get,
    path = "/api/strategies/{id}/indicators/{indicator_id}",
    tag = "custom_indicators",
    params(
        ("id" = Uuid, Path, description = "戦略 ID"),
        ("indicator_id" = Uuid, Path, description = "indicator ID"),
    ),
    responses(
        (status = 200, body = custom_indicator::Model),
        (status = 404, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn get_strategy_indicator(
    State(state): State<AppState>,
    JsonPath((strategy_id, indicator_id)): JsonPath<(Uuid, Uuid)>,
) -> Result<Json<custom_indicator::Model>, AppError> {
    ensure_strategy_exists(&state.db, strategy_id).await?;
    Ok(Json(
        find_strategy_scoped_or_404(&state.db, strategy_id, indicator_id).await?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::create_test_server;
    use serde_json::json;
    use sqlx::PgPool;

    async fn create_strategy(server: &axum_test::TestServer, name: &str) -> Uuid {
        let res = server
            .post("/api/strategies")
            .json(&json!({ "name": name }))
            .await;
        res.assert_status(StatusCode::CREATED);
        let id = res.json::<serde_json::Value>()["id"]
            .as_str()
            .map(str::to_string)
            .expect("id");
        Uuid::parse_str(&id).expect("uuid")
    }

    fn create_payload(name: &str, code: &str) -> serde_json::Value {
        json!({
            "name": name,
            "code": code,
            "input_schema": {"type": "object"},
            "output_schema": {"type": "object"},
        })
    }

    fn normalize_dynamic(body: &mut serde_json::Value) {
        for key in ["indicator_id", "created_at", "updated_at"] {
            if body.get(key).is_some() {
                body[key] = json!("<dyn>");
            }
        }
    }

    #[sqlx::test(migrations = false)]
    async fn create_global_indicator_returns_201(pool: PgPool) {
        let server = create_test_server(pool).await;
        let res = server
            .post("/api/indicators")
            .json(&create_payload("rsi", "print('{}')"))
            .await;
        res.assert_status(StatusCode::CREATED);
        let mut body = res.json::<serde_json::Value>();
        normalize_dynamic(&mut body);
        assert_eq!(
            body,
            json!({
                "indicator_id": "<dyn>",
                "name": "rsi",
                "scope": "global",
                "strategy_id": null,
                "code": "print('{}')",
                "input_schema": {"type": "object"},
                "output_schema": {"type": "object"},
                "description": null,
                "created_at": "<dyn>",
                "updated_at": "<dyn>",
            }),
        );
    }

    #[sqlx::test(migrations = false)]
    async fn create_strategy_indicator_returns_201(pool: PgPool) {
        let server = create_test_server(pool).await;
        let strategy_id = create_strategy(&server, "s1").await;

        let res = server
            .post(&format!("/api/strategies/{strategy_id}/indicators"))
            .json(&create_payload("rsi", "print('{}')"))
            .await;
        res.assert_status(StatusCode::CREATED);
        let mut body = res.json::<serde_json::Value>();
        normalize_dynamic(&mut body);
        assert_eq!(
            body,
            json!({
                "indicator_id": "<dyn>",
                "name": "rsi",
                "scope": "strategy",
                "strategy_id": strategy_id.to_string(),
                "code": "print('{}')",
                "input_schema": {"type": "object"},
                "output_schema": {"type": "object"},
                "description": null,
                "created_at": "<dyn>",
                "updated_at": "<dyn>",
            }),
        );
    }

    #[sqlx::test(migrations = false)]
    async fn empty_name_returns_400(pool: PgPool) {
        let server = create_test_server(pool).await;
        let res = server
            .post("/api/indicators")
            .json(&create_payload("   ", "print('{}')"))
            .await;
        res.assert_status(StatusCode::BAD_REQUEST);
    }

    #[sqlx::test(migrations = false)]
    async fn input_schema_non_object_returns_400(pool: PgPool) {
        let server = create_test_server(pool).await;
        let mut payload = create_payload("rsi", "print('{}')");
        payload["input_schema"] = json!([1, 2, 3]);
        let res = server.post("/api/indicators").json(&payload).await;
        res.assert_status(StatusCode::BAD_REQUEST);
    }

    #[sqlx::test(migrations = false)]
    async fn output_schema_non_object_returns_400(pool: PgPool) {
        let server = create_test_server(pool).await;
        let mut payload = create_payload("rsi", "print('{}')");
        payload["output_schema"] = json!("not-object");
        let res = server.post("/api/indicators").json(&payload).await;
        res.assert_status(StatusCode::BAD_REQUEST);
    }

    #[sqlx::test(migrations = false)]
    async fn duplicate_global_name_returns_409(pool: PgPool) {
        let server = create_test_server(pool).await;
        let payload = create_payload("rsi", "print('{}')");
        server.post("/api/indicators").json(&payload).await;
        let second = server.post("/api/indicators").json(&payload).await;
        second.assert_status(StatusCode::CONFLICT);
    }

    #[sqlx::test(migrations = false)]
    async fn duplicate_strategy_name_returns_409(pool: PgPool) {
        let server = create_test_server(pool).await;
        let strategy_id = create_strategy(&server, "s1").await;
        let payload = create_payload("rsi", "print('{}')");
        server
            .post(&format!("/api/strategies/{strategy_id}/indicators"))
            .json(&payload)
            .await;
        let second = server
            .post(&format!("/api/strategies/{strategy_id}/indicators"))
            .json(&payload)
            .await;
        second.assert_status(StatusCode::CONFLICT);
    }

    #[sqlx::test(migrations = false)]
    async fn global_and_strategy_can_share_name(pool: PgPool) {
        let server = create_test_server(pool).await;
        let strategy_id = create_strategy(&server, "s1").await;
        let payload = create_payload("rsi", "print('{}')");

        let g = server.post("/api/indicators").json(&payload).await;
        g.assert_status(StatusCode::CREATED);

        let s = server
            .post(&format!("/api/strategies/{strategy_id}/indicators"))
            .json(&payload)
            .await;
        s.assert_status(StatusCode::CREATED);
    }

    #[sqlx::test(migrations = false)]
    async fn list_isolates_strategy_scopes(pool: PgPool) {
        let server = create_test_server(pool).await;
        let s_a = create_strategy(&server, "a").await;
        let s_b = create_strategy(&server, "b").await;
        server
            .post(&format!("/api/strategies/{s_a}/indicators"))
            .json(&create_payload("only-a", "print('{}')"))
            .await;
        server
            .post(&format!("/api/strategies/{s_b}/indicators"))
            .json(&create_payload("only-b", "print('{}')"))
            .await;
        server
            .post("/api/indicators")
            .json(&create_payload("global", "print('{}')"))
            .await;

        let list_a = server
            .get(&format!("/api/strategies/{s_a}/indicators"))
            .await;
        list_a.assert_status_ok();
        let body_a: Vec<serde_json::Value> = list_a.json();
        let names_a: Vec<&str> = body_a.iter().map(|v| v["name"].as_str().unwrap()).collect();
        assert_eq!(names_a, vec!["only-a"]);

        let globals = server.get("/api/indicators").await;
        globals.assert_status_ok();
        let body_g: Vec<serde_json::Value> = globals.json();
        let names_g: Vec<&str> = body_g.iter().map(|v| v["name"].as_str().unwrap()).collect();
        assert_eq!(names_g, vec!["global"]);
    }

    #[sqlx::test(migrations = false)]
    async fn get_strategy_indicator_from_other_strategy_returns_404(pool: PgPool) {
        let server = create_test_server(pool).await;
        let s_a = create_strategy(&server, "a").await;
        let s_b = create_strategy(&server, "b").await;
        let created = server
            .post(&format!("/api/strategies/{s_a}/indicators"))
            .json(&create_payload("only-a", "print('{}')"))
            .await;
        let id = created.json::<serde_json::Value>()["indicator_id"]
            .as_str()
            .map(str::to_string)
            .expect("id");

        let res = server
            .get(&format!("/api/strategies/{s_b}/indicators/{id}"))
            .await;
        res.assert_status(StatusCode::NOT_FOUND);
    }

    #[sqlx::test(migrations = false)]
    async fn update_changes_fields(pool: PgPool) {
        let server = create_test_server(pool).await;
        let created = server
            .post("/api/indicators")
            .json(&create_payload("rsi", "old"))
            .await;
        let id = created.json::<serde_json::Value>()["indicator_id"]
            .as_str()
            .map(str::to_string)
            .expect("id");

        let updated = server
            .put(&format!("/api/indicators/{id}"))
            .json(&json!({"code": "new", "description": "rsi indicator"}))
            .await;
        updated.assert_status_ok();
        let mut body = updated.json::<serde_json::Value>();
        normalize_dynamic(&mut body);
        assert_eq!(
            body,
            json!({
                "indicator_id": "<dyn>",
                "name": "rsi",
                "scope": "global",
                "strategy_id": null,
                "code": "new",
                "input_schema": {"type": "object"},
                "output_schema": {"type": "object"},
                "description": "rsi indicator",
                "created_at": "<dyn>",
                "updated_at": "<dyn>",
            }),
        );
    }

    #[sqlx::test(migrations = false)]
    async fn delete_removes_indicator(pool: PgPool) {
        let server = create_test_server(pool).await;
        let created = server
            .post("/api/indicators")
            .json(&create_payload("rsi", "print('{}')"))
            .await;
        let id = created.json::<serde_json::Value>()["indicator_id"]
            .as_str()
            .map(str::to_string)
            .expect("id");

        let del = server.delete(&format!("/api/indicators/{id}")).await;
        del.assert_status(StatusCode::NO_CONTENT);
        let get = server.get(&format!("/api/indicators/{id}")).await;
        get.assert_status(StatusCode::NOT_FOUND);
    }

    #[sqlx::test(migrations = false)]
    async fn resolve_indicator_prefers_strategy_scope(pool: PgPool) {
        use crate::services::custom_indicators::resolve_indicator;
        use crate::testing::create_test_db;

        let db = create_test_db(pool).await;

        let strategy_id = Uuid::new_v4();
        let now = chrono::Utc::now().fixed_offset();
        crate::entities::strategy::ActiveModel {
            id: Set(strategy_id),
            name: Set("s".into()),
            description: Set(None),
            sort_order: Set(0),
            agents_md: NotSet,
            skills: NotSet,
            agent_status: NotSet,
            agent_error: NotSet,
            created_at: Set(now),
            updated_at: Set(now),
        }
        .insert(&db)
        .await
        .unwrap();

        let mk = |scope: &str, sid: Option<Uuid>, name: &str, code: &str| {
            custom_indicator::ActiveModel {
                indicator_id: Set(Uuid::new_v4()),
                name: Set(name.into()),
                scope: Set(scope.into()),
                strategy_id: Set(sid),
                code: Set(code.into()),
                input_schema: Set(json!({})),
                output_schema: Set(json!({})),
                description: Set(None),
                created_at: NotSet,
                updated_at: NotSet,
            }
        };

        custom_indicator::Entity::insert(mk(SCOPE_GLOBAL, None, "rsi", "global-code"))
            .exec(&db)
            .await
            .unwrap();
        custom_indicator::Entity::insert(mk(
            SCOPE_STRATEGY,
            Some(strategy_id),
            "rsi",
            "strategy-code",
        ))
        .exec(&db)
        .await
        .unwrap();
        custom_indicator::Entity::insert(mk(SCOPE_GLOBAL, None, "global-only", "g"))
            .exec(&db)
            .await
            .unwrap();

        let resolved = resolve_indicator(&db, strategy_id, "rsi")
            .await
            .unwrap()
            .expect("resolved");
        assert_eq!(resolved.code, "strategy-code");

        let resolved_global = resolve_indicator(&db, strategy_id, "global-only")
            .await
            .unwrap()
            .expect("resolved");
        assert_eq!(resolved_global.code, "g");

        let unresolved = resolve_indicator(&db, strategy_id, "missing")
            .await
            .unwrap();
        assert!(unresolved.is_none());
    }
}
