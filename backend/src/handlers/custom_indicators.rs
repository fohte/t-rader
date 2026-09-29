use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use core_application::change_history::ChangeHistoryError;
use core_application::custom_indicator::{
    CreateCustomIndicatorCommand, CustomIndicatorRepositoryError, CustomIndicatorUseCaseError,
    UpdateCustomIndicatorCommand,
};
use core_application::strategy_existence::StrategyExistenceError;
use core_application::unit_of_work::UnitOfWorkError;
use uuid::Uuid;

use crate::AppState;
use crate::error::{AppError, ErrorResponse};
use crate::extractors::{JsonBody, JsonPath};
use crate::models::{
    CreateCustomIndicatorRequest, CustomIndicatorResponse, PreviewIndicatorRequest,
    PreviewIndicatorResponse, UpdateCustomIndicatorRequest,
};
use crate::services::custom_indicators::{PreviewInput, run_preview};

fn ensure_json_object(field: &str, value: &serde_json::Value) -> Result<(), AppError> {
    if value.is_object() {
        Ok(())
    } else {
        Err(AppError::Validation(format!(
            "{field} must be a JSON object"
        )))
    }
}

/// グローバル indicator 一覧
#[utoipa::path(
    get,
    path = "/api/indicators",
    tag = "custom_indicators",
    responses(
        (status = 200, body = Vec<CustomIndicatorResponse>),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn list_global_indicators(
    State(state): State<AppState>,
) -> Result<Json<Vec<CustomIndicatorResponse>>, AppError> {
    let items = state
        .use_cases
        .custom_indicators
        .list_global()
        .await
        .map_err(map_custom_indicator_error)?;
    Ok(Json(items.into_iter().map(Into::into).collect()))
}

/// 戦略 scope indicator 一覧
#[utoipa::path(
    get,
    path = "/api/strategies/{id}/indicators",
    tag = "custom_indicators",
    params(("id" = Uuid, Path, description = "戦略 ID")),
    responses(
        (status = 200, body = Vec<CustomIndicatorResponse>),
        (status = 400, description = "リクエストパラメータが不正", body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn list_strategy_indicators(
    State(state): State<AppState>,
    JsonPath(strategy_id): JsonPath<Uuid>,
) -> Result<Json<Vec<CustomIndicatorResponse>>, AppError> {
    let items = state
        .use_cases
        .custom_indicators
        .list_strategy(strategy_id)
        .await
        .map_err(map_custom_indicator_error)?;
    Ok(Json(items.into_iter().map(Into::into).collect()))
}

/// indicator 詳細
#[utoipa::path(
    get,
    path = "/api/indicators/{indicator_id}",
    tag = "custom_indicators",
    params(("indicator_id" = Uuid, Path, description = "indicator ID")),
    responses(
        (status = 200, body = CustomIndicatorResponse),
        (status = 400, description = "リクエストパラメータが不正", body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn get_indicator(
    State(state): State<AppState>,
    JsonPath(indicator_id): JsonPath<Uuid>,
) -> Result<Json<CustomIndicatorResponse>, AppError> {
    let indicator = state
        .use_cases
        .custom_indicators
        .get(indicator_id)
        .await
        .map_err(map_custom_indicator_error)?;
    Ok(Json(indicator.into()))
}

/// グローバル indicator 作成
#[utoipa::path(
    post,
    path = "/api/indicators",
    tag = "custom_indicators",
    request_body = CreateCustomIndicatorRequest,
    responses(
        (status = 201, body = CustomIndicatorResponse),
        (status = 400, description = "リクエストパラメータが不正", body = ErrorResponse),
        (status = 409, body = ErrorResponse),
        (status = 415, description = "Content-Type ヘッダが application/json ではない", body = ErrorResponse),
        (status = 422, description = "リクエストボディのパースに失敗", body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn create_global_indicator(
    State(state): State<AppState>,
    JsonBody(payload): JsonBody<CreateCustomIndicatorRequest>,
) -> Result<(StatusCode, Json<CustomIndicatorResponse>), AppError> {
    let indicator = state
        .use_cases
        .custom_indicators
        .create(CreateCustomIndicatorCommand {
            name: payload.name,
            strategy_id: None,
            code: payload.code,
            input_schema: payload.input_schema,
            output_schema: payload.output_schema,
            description: payload.description,
        })
        .await
        .map_err(map_custom_indicator_error)?;
    Ok((StatusCode::CREATED, Json(indicator.into())))
}

/// 戦略 scope indicator 作成
#[utoipa::path(
    post,
    path = "/api/strategies/{id}/indicators",
    tag = "custom_indicators",
    params(("id" = Uuid, Path, description = "戦略 ID")),
    request_body = CreateCustomIndicatorRequest,
    responses(
        (status = 201, body = CustomIndicatorResponse),
        (status = 400, description = "リクエストパラメータが不正", body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 409, body = ErrorResponse),
        (status = 415, description = "Content-Type ヘッダが application/json ではない", body = ErrorResponse),
        (status = 422, description = "リクエストボディのパースに失敗", body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn create_strategy_indicator(
    State(state): State<AppState>,
    JsonPath(strategy_id): JsonPath<Uuid>,
    JsonBody(payload): JsonBody<CreateCustomIndicatorRequest>,
) -> Result<(StatusCode, Json<CustomIndicatorResponse>), AppError> {
    let indicator = state
        .use_cases
        .custom_indicators
        .create(CreateCustomIndicatorCommand {
            name: payload.name,
            strategy_id: Some(strategy_id),
            code: payload.code,
            input_schema: payload.input_schema,
            output_schema: payload.output_schema,
            description: payload.description,
        })
        .await
        .map_err(map_custom_indicator_error)?;
    Ok((StatusCode::CREATED, Json(indicator.into())))
}

/// indicator 更新
#[utoipa::path(
    put,
    path = "/api/indicators/{indicator_id}",
    tag = "custom_indicators",
    params(("indicator_id" = Uuid, Path, description = "indicator ID")),
    request_body = UpdateCustomIndicatorRequest,
    responses(
        (status = 200, body = CustomIndicatorResponse),
        (status = 400, description = "リクエストパラメータが不正", body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 409, body = ErrorResponse),
        (status = 415, description = "Content-Type ヘッダが application/json ではない", body = ErrorResponse),
        (status = 422, description = "リクエストボディのパースに失敗", body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn update_indicator(
    State(state): State<AppState>,
    JsonPath(indicator_id): JsonPath<Uuid>,
    JsonBody(payload): JsonBody<UpdateCustomIndicatorRequest>,
) -> Result<Json<CustomIndicatorResponse>, AppError> {
    let indicator = state
        .use_cases
        .custom_indicators
        .update(
            indicator_id,
            UpdateCustomIndicatorCommand {
                name: payload.name,
                code: payload.code,
                input_schema: payload.input_schema,
                output_schema: payload.output_schema,
                description: payload.description,
            },
        )
        .await
        .map_err(map_custom_indicator_error)?;
    Ok(Json(indicator.into()))
}

/// indicator 削除
#[utoipa::path(
    delete,
    path = "/api/indicators/{indicator_id}",
    tag = "custom_indicators",
    params(("indicator_id" = Uuid, Path, description = "indicator ID")),
    responses(
        (status = 204),
        (status = 400, description = "リクエストパラメータが不正", body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn delete_indicator(
    State(state): State<AppState>,
    JsonPath(indicator_id): JsonPath<Uuid>,
) -> Result<StatusCode, AppError> {
    state
        .use_cases
        .custom_indicators
        .delete(indicator_id)
        .await
        .map_err(map_custom_indicator_error)?;
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
        (status = 200, body = CustomIndicatorResponse),
        (status = 400, description = "リクエストパラメータが不正", body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn get_strategy_indicator(
    State(state): State<AppState>,
    JsonPath((strategy_id, indicator_id)): JsonPath<(Uuid, Uuid)>,
) -> Result<Json<CustomIndicatorResponse>, AppError> {
    let indicator = state
        .use_cases
        .custom_indicators
        .get_strategy_indicator(strategy_id, indicator_id)
        .await
        .map_err(map_custom_indicator_error)?;
    Ok(Json(indicator.into()))
}

fn map_custom_indicator_error(error: CustomIndicatorUseCaseError) -> AppError {
    match error {
        CustomIndicatorUseCaseError::Validation(message) => AppError::Validation(message),
        CustomIndicatorUseCaseError::NotFound(indicator_id) => {
            AppError::NotFound(format!("indicator {indicator_id} not found"))
        }
        CustomIndicatorUseCaseError::Repository(CustomIndicatorRepositoryError::Database(
            error,
        ))
        | CustomIndicatorUseCaseError::ChangeHistory(ChangeHistoryError::Database(error))
        | CustomIndicatorUseCaseError::UnitOfWork(UnitOfWorkError::Begin(error))
        | CustomIndicatorUseCaseError::UnitOfWork(UnitOfWorkError::Commit(error))
        | CustomIndicatorUseCaseError::StrategyExistence(StrategyExistenceError::Database(error)) => {
            error.into()
        }
        other => AppError::Database(sea_orm::DbErr::Custom(other.to_string())),
    }
}

/// indicator を保存せずに 1 回だけ実行してみる (Monaco エディタのプレビュー用)
#[utoipa::path(
    post,
    path = "/api/indicators/preview",
    tag = "custom_indicators",
    request_body = PreviewIndicatorRequest,
    responses(
        (status = 200, body = PreviewIndicatorResponse),
        (status = 400, description = "code / schema / args が不正", body = ErrorResponse),
        (status = 415, description = "Content-Type ヘッダが application/json ではない", body = ErrorResponse),
        (status = 422, description = "リクエストボディのパースに失敗", body = ErrorResponse),
        (status = 503, description = "indicator runtime が未設定または利用不可", body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn preview_indicator(
    State(state): State<AppState>,
    JsonBody(payload): JsonBody<PreviewIndicatorRequest>,
) -> Result<Json<PreviewIndicatorResponse>, AppError> {
    ensure_json_object("input_schema", &payload.input_schema)?;
    ensure_json_object("output_schema", &payload.output_schema)?;

    let executor = state.kata_executor.as_ref().ok_or_else(|| {
        AppError::ServiceUnavailable("indicator runtime is not configured".into())
    })?;

    let outcome = run_preview(
        executor,
        PreviewInput {
            code: &payload.code,
            input_schema: &payload.input_schema,
            output_schema: &payload.output_schema,
            args: &payload.args,
            timeout_secs: payload.timeout_secs,
            max_output_bytes: payload.max_output_bytes,
        },
    )
    .await?;

    Ok(Json(PreviewIndicatorResponse {
        output: outcome.output,
        stdout: outcome.stdout,
        stderr: outcome.stderr,
        exit_code: outcome.exit_code,
    }))
}

#[cfg(test)]
mod tests {
    use sea_orm::{ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, QueryFilter};

    use super::*;
    use crate::testing::{create_test_server, create_test_server_with_db};
    use gateway_postgres::entities::change_history;
    use serde_json::json;
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

    /// 指定 indicator の最新の change_history 行を返す (`/api/history` は created_at 降順)
    async fn latest_history_for(
        server: &axum_test::TestServer,
        indicator_id: &str,
    ) -> serde_json::Value {
        let res = server
            .get(&format!(
                "/api/history?target_kind=custom_indicator&target_id={indicator_id}"
            ))
            .await;
        res.assert_status_ok();
        let mut rows = res.json::<Vec<serde_json::Value>>();
        assert!(!rows.is_empty(), "expected at least one history row");
        let mut row = rows.remove(0);
        for key in ["id", "created_at"] {
            row[key] = json!("<dyn>");
        }
        row
    }

    async fn set_history_created_at_to_epoch(db: &impl sea_orm::ConnectionTrait, target_id: &str) {
        let row = change_history::Entity::find()
            .filter(
                change_history::Column::TargetId
                    .eq(Uuid::parse_str(target_id).expect("indicator UUID")),
            )
            .one(db)
            .await
            .expect("find history row")
            .expect("history row exists");
        change_history::ActiveModel {
            id: Set(row.id),
            created_at: Set(chrono::DateTime::<chrono::Utc>::UNIX_EPOCH.fixed_offset()),
            ..Default::default()
        }
        .update(db)
        .await
        .expect("set history row time");
    }

    #[backend_test_macros::database_test]
    async fn create_global_indicator_returns_201(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
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

    #[backend_test_macros::database_test]
    async fn create_strategy_indicator_returns_201(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
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

    #[backend_test_macros::database_test]
    async fn create_records_change_history(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        let res = server
            .post("/api/indicators")
            .json(&create_payload("rsi", "print('{}')"))
            .await;
        let id = res.json::<serde_json::Value>()["indicator_id"]
            .as_str()
            .map(str::to_string)
            .expect("id");

        assert_eq!(
            latest_history_for(&server, &id).await,
            json!({
                "id": "<dyn>",
                "target_kind": "custom_indicator",
                "target_id": id,
                "actor_kind": "human",
                "actor_label": "user",
                "op": "create",
                "diff_json": {"name": "rsi", "scope": "global", "strategy_id": null},
                "summary": null,
                "created_at": "<dyn>",
            }),
        );
    }

    #[backend_test_macros::database_test]
    async fn empty_name_returns_400(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        let res = server
            .post("/api/indicators")
            .json(&create_payload("   ", "print('{}')"))
            .await;
        res.assert_status(StatusCode::BAD_REQUEST);
    }

    #[backend_test_macros::database_test]
    async fn input_schema_non_object_returns_400(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        let mut payload = create_payload("rsi", "print('{}')");
        payload["input_schema"] = json!([1, 2, 3]);
        let res = server.post("/api/indicators").json(&payload).await;
        res.assert_status(StatusCode::BAD_REQUEST);
    }

    #[backend_test_macros::database_test]
    async fn output_schema_non_object_returns_400(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        let mut payload = create_payload("rsi", "print('{}')");
        payload["output_schema"] = json!("not-object");
        let res = server.post("/api/indicators").json(&payload).await;
        res.assert_status(StatusCode::BAD_REQUEST);
    }

    #[backend_test_macros::database_test]
    async fn duplicate_global_name_returns_409(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        let payload = create_payload("rsi", "print('{}')");
        server.post("/api/indicators").json(&payload).await;
        let second = server.post("/api/indicators").json(&payload).await;
        second.assert_status(StatusCode::CONFLICT);
    }

    #[backend_test_macros::database_test]
    async fn duplicate_strategy_name_returns_409(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
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

    #[backend_test_macros::database_test]
    async fn global_and_strategy_can_share_name(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
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

    #[backend_test_macros::database_test]
    async fn list_isolates_strategy_scopes(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
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

    #[backend_test_macros::database_test]
    async fn get_strategy_indicator_from_other_strategy_returns_404(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let server = create_test_server(db).await;
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

    #[backend_test_macros::database_test]
    async fn update_changes_fields(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
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

    #[backend_test_macros::database_test]
    async fn update_records_change_history_diff(db: gateway_postgres::DatabaseHandle) {
        let (db, server) = create_test_server_with_db(db).await;
        let created = server
            .post("/api/indicators")
            .json(&create_payload("rsi", "old"))
            .await;
        let id = created.json::<serde_json::Value>()["indicator_id"]
            .as_str()
            .map(str::to_string)
            .expect("id");
        set_history_created_at_to_epoch(&db, &id).await;

        server
            .put(&format!("/api/indicators/{id}"))
            .json(&json!({"code": "new", "description": "rsi indicator"}))
            .await
            .assert_status_ok();

        assert_eq!(
            latest_history_for(&server, &id).await,
            json!({
                "id": "<dyn>",
                "target_kind": "custom_indicator",
                "target_id": id,
                "actor_kind": "human",
                "actor_label": "user",
                "op": "update",
                "diff_json": {
                    "code": {"len_from": 3, "len_to": 3},
                    "description": {"from": null, "to": "rsi indicator"},
                },
                "summary": null,
                "created_at": "<dyn>",
            }),
        );
    }

    #[backend_test_macros::database_test]
    async fn delete_removes_indicator(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
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

    #[backend_test_macros::database_test]
    async fn delete_records_change_history(db: gateway_postgres::DatabaseHandle) {
        let (db, server) = create_test_server_with_db(db).await;
        let created = server
            .post("/api/indicators")
            .json(&create_payload("rsi", "print('{}')"))
            .await;
        let id = created.json::<serde_json::Value>()["indicator_id"]
            .as_str()
            .map(str::to_string)
            .expect("id");
        set_history_created_at_to_epoch(&db, &id).await;

        server
            .delete(&format!("/api/indicators/{id}"))
            .await
            .assert_status(StatusCode::NO_CONTENT);

        assert_eq!(
            latest_history_for(&server, &id).await,
            json!({
                "id": "<dyn>",
                "target_kind": "custom_indicator",
                "target_id": id,
                "actor_kind": "human",
                "actor_label": "user",
                "op": "delete",
                "diff_json": {},
                "summary": null,
                "created_at": "<dyn>",
            }),
        );
    }

    mod preview {
        use super::*;
        use crate::kata_exec::{ExecResult, FakeKataExecutor, SharedKataExecutor};
        use crate::testing::create_test_server_with_kata;
        use std::sync::Arc;

        fn preview_payload(code: &str) -> serde_json::Value {
            json!({
                "code": code,
                "input_schema": {"type": "object"},
                "output_schema": {"type": "object"},
                "args": {},
            })
        }

        #[backend_test_macros::database_test]
        async fn preview_returns_validated_output(db: gateway_postgres::DatabaseHandle) {
            let executor = Arc::new(FakeKataExecutor::new());
            executor
                .set_response(Ok(ExecResult {
                    stdout: "{\"value\": 42}\n".into(),
                    stderr: String::new(),
                    exit_code: 0,
                }))
                .await;
            let shared: SharedKataExecutor = executor.clone();
            let server = create_test_server_with_kata(db, shared).await;

            let res = server
                .post("/api/indicators/preview")
                .json(&json!({
                    "code": "print('{\"value\": 42}')",
                    "input_schema": {"type": "object", "properties": {"period": {"type": "integer"}}, "required": ["period"]},
                    "output_schema": {"type": "object", "properties": {"value": {"type": "number"}}, "required": ["value"]},
                    "args": {"period": 14},
                }))
                .await;
            res.assert_status_ok();
            assert_eq!(
                res.json::<serde_json::Value>(),
                json!({
                    "output": {"value": 42},
                    "stdout": "{\"value\": 42}\n",
                    "stderr": "",
                    "exit_code": 0,
                }),
            );

            let recorded = executor.requests.lock().await;
            assert_eq!(
                recorded.as_slice(),
                &[crate::kata_exec::ExecRequest {
                    code: "print('{\"value\": 42}')".into(),
                    stdin: Some(r#"{"args":{"period":14}}"#.into()),
                    timeout: None,
                    max_output_bytes: None,
                }],
            );
        }

        #[backend_test_macros::database_test]
        async fn preview_returns_400_for_input_schema_mismatch(
            db: gateway_postgres::DatabaseHandle,
        ) {
            let executor = Arc::new(FakeKataExecutor::new());
            let shared: SharedKataExecutor = executor.clone();
            let server = create_test_server_with_kata(db, shared).await;

            let res = server
                .post("/api/indicators/preview")
                .json(&json!({
                    "code": "print('{}')",
                    "input_schema": {
                        "type": "object",
                        "properties": {"period": {"type": "integer"}},
                        "required": ["period"],
                    },
                    "output_schema": {"type": "object"},
                    "args": {"period": "not-int"},
                }))
                .await;
            res.assert_status(StatusCode::BAD_REQUEST);
            assert!(executor.requests.lock().await.is_empty());
        }

        #[backend_test_macros::database_test]
        async fn preview_passes_through_sandbox_rejection(db: gateway_postgres::DatabaseHandle) {
            let executor = Arc::new(FakeKataExecutor::new());
            executor
                .set_response(Ok(ExecResult {
                    stdout: String::new(),
                    stderr: "PermissionError: network access denied".into(),
                    exit_code: 1,
                }))
                .await;
            let shared: SharedKataExecutor = executor;
            let server = create_test_server_with_kata(db, shared).await;

            let res = server
                .post("/api/indicators/preview")
                .json(&preview_payload(
                    "import urllib.request; urllib.request.urlopen('http://x')",
                ))
                .await;
            res.assert_status_ok();
            assert_eq!(
                res.json::<serde_json::Value>(),
                json!({
                    "output": null,
                    "stdout": "",
                    "stderr": "PermissionError: network access denied",
                    "exit_code": 1,
                }),
            );
        }

        #[backend_test_macros::database_test]
        async fn preview_returns_503_when_executor_disabled(db: gateway_postgres::DatabaseHandle) {
            let server = create_test_server(db).await;
            let res = server
                .post("/api/indicators/preview")
                .json(&preview_payload("print('{}')"))
                .await;
            res.assert_status(StatusCode::SERVICE_UNAVAILABLE);
        }

        #[backend_test_macros::database_test]
        async fn preview_returns_200_with_validation_error_in_stderr_for_invalid_output(
            db: gateway_postgres::DatabaseHandle,
        ) {
            let executor = Arc::new(FakeKataExecutor::new());
            executor
                .set_response(Ok(ExecResult {
                    stdout: "not-json\n".into(),
                    stderr: String::new(),
                    exit_code: 0,
                }))
                .await;
            let shared: SharedKataExecutor = executor;
            let server = create_test_server_with_kata(db, shared).await;

            let res = server
                .post("/api/indicators/preview")
                .json(&preview_payload("print('not-json')"))
                .await;
            res.assert_status_ok();
            let mut body = res.json::<serde_json::Value>();
            // stderr の本文は jsonschema の詳細メッセージに依存して変動するため、
            // prefix が出ているかだけを spec として固定する。
            let stderr_starts_correctly = body["stderr"]
                .as_str()
                .is_some_and(|s| s.starts_with("Output validation error: "));
            body["stderr"] = json!(stderr_starts_correctly);
            assert_eq!(
                body,
                json!({
                    "output": null,
                    "stdout": "not-json\n",
                    "stderr": true,
                    "exit_code": 0,
                }),
            );
        }
    }
}
