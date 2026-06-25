use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use sea_orm::ActiveModelTrait;
use sea_orm::ActiveValue::{NotSet, Set};
use sea_orm::{ColumnTrait, EntityTrait, IntoActiveModel, QueryFilter, QueryOrder};
use uuid::Uuid;

use crate::AppState;
use crate::entities::trigger;
use crate::error::{AppError, ErrorResponse};
use crate::extractors::{JsonBody, JsonPath, JsonQuery};
use crate::models::{CreateTriggerRequest, ListTriggersQuery, TriggerKind, UpdateTriggerRequest};

fn validate_template(value: &str) -> Result<String, AppError> {
    let trimmed = value.trim().to_string();
    if trimmed.is_empty() {
        return Err(AppError::Validation(
            "prompt_template must not be empty".into(),
        ));
    }
    Ok(trimmed)
}

fn validate_create(payload: &CreateTriggerRequest) -> Result<(), AppError> {
    match payload.kind {
        TriggerKind::Cron => {
            if payload
                .schedule
                .as_deref()
                .map(str::trim)
                .map(str::is_empty)
                .unwrap_or(true)
            {
                return Err(AppError::Validation(
                    "schedule is required for kind=cron".into(),
                ));
            }
            if payload.hook_slug.is_some() {
                return Err(AppError::Validation(
                    "hook_slug must be omitted for kind=cron".into(),
                ));
            }
        }
        TriggerKind::Hook => {
            if payload
                .hook_slug
                .as_deref()
                .map(str::trim)
                .map(str::is_empty)
                .unwrap_or(true)
            {
                return Err(AppError::Validation(
                    "hook_slug is required for kind=hook".into(),
                ));
            }
            if payload.schedule.is_some() {
                return Err(AppError::Validation(
                    "schedule must be omitted for kind=hook".into(),
                ));
            }
        }
    }
    Ok(())
}

async fn find_strategy_or_404(db: &sea_orm::DatabaseConnection, id: Uuid) -> Result<(), AppError> {
    let exists = crate::entities::strategy::Entity::find_by_id(id)
        .one(db)
        .await?
        .is_some();
    if !exists {
        return Err(AppError::NotFound(format!("strategy {id} not found")));
    }
    Ok(())
}

async fn find_trigger_or_404(
    db: &sea_orm::DatabaseConnection,
    trigger_id: Uuid,
) -> Result<trigger::Model, AppError> {
    trigger::Entity::find_by_id(trigger_id)
        .one(db)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("trigger {trigger_id} not found")))
}

/// 戦略の trigger 一覧
#[utoipa::path(
    get,
    path = "/api/strategies/{id}/triggers",
    tag = "triggers",
    params(
        ("id" = Uuid, Path, description = "戦略 ID"),
        ("kind" = Option<String>, Query, description = "kind フィルタ (cron|hook)"),
    ),
    responses(
        (status = 200, body = Vec<trigger::Model>),
        (status = 400, description = "リクエストパラメータが不正", body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn list_strategy_triggers(
    State(state): State<AppState>,
    JsonPath(id): JsonPath<Uuid>,
    JsonQuery(query): JsonQuery<ListTriggersQuery>,
) -> Result<Json<Vec<trigger::Model>>, AppError> {
    find_strategy_or_404(&state.db, id).await?;
    let mut find = trigger::Entity::find().filter(trigger::Column::StrategyId.eq(id));
    if let Some(kind) = query.kind {
        find = find.filter(trigger::Column::Kind.eq(kind.as_str()));
    }
    let items = find
        .order_by_asc(trigger::Column::CreatedAt)
        .all(&state.db)
        .await?;
    Ok(Json(items))
}

/// trigger を作成
#[utoipa::path(
    post,
    path = "/api/strategies/{id}/triggers",
    tag = "triggers",
    params(("id" = Uuid, Path, description = "戦略 ID")),
    request_body = CreateTriggerRequest,
    responses(
        (status = 201, body = trigger::Model),
        (status = 400, description = "リクエストパラメータが不正", body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 409, description = "hook_slug が他 trigger と衝突", body = ErrorResponse),
        (status = 422, description = "リクエストボディのパースに失敗", body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn create_strategy_trigger(
    State(state): State<AppState>,
    JsonPath(strategy_id): JsonPath<Uuid>,
    JsonBody(payload): JsonBody<CreateTriggerRequest>,
) -> Result<(StatusCode, Json<trigger::Model>), AppError> {
    find_strategy_or_404(&state.db, strategy_id).await?;
    validate_create(&payload)?;
    let prompt_template = validate_template(&payload.prompt_template)?;

    let model = trigger::ActiveModel {
        trigger_id: Set(Uuid::new_v4()),
        strategy_id: Set(strategy_id),
        kind: Set(payload.kind.as_str().to_string()),
        schedule: Set(payload.schedule.map(|s| s.trim().to_string())),
        hook_slug: Set(payload.hook_slug.map(|s| s.trim().to_string())),
        event_match: Set(payload.event_match),
        prompt_template: Set(prompt_template),
        enabled: Set(payload.enabled.unwrap_or(true)),
        last_fired_at: NotSet,
        created_at: NotSet,
        updated_at: NotSet,
    };
    let created = trigger::Entity::insert(model)
        .exec_with_returning(&state.db)
        .await?;
    Ok((StatusCode::CREATED, Json(created)))
}

/// trigger 詳細
#[utoipa::path(
    get,
    path = "/api/triggers/{trigger_id}",
    tag = "triggers",
    params(("trigger_id" = Uuid, Path, description = "trigger ID")),
    responses(
        (status = 200, body = trigger::Model),
        (status = 400, description = "リクエストパラメータが不正", body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn get_trigger(
    State(state): State<AppState>,
    JsonPath(trigger_id): JsonPath<Uuid>,
) -> Result<Json<trigger::Model>, AppError> {
    let model = find_trigger_or_404(&state.db, trigger_id).await?;
    Ok(Json(model))
}

/// trigger 更新 (kind / strategy_id は不変)
#[utoipa::path(
    put,
    path = "/api/triggers/{trigger_id}",
    tag = "triggers",
    params(("trigger_id" = Uuid, Path, description = "trigger ID")),
    request_body = UpdateTriggerRequest,
    responses(
        (status = 200, body = trigger::Model),
        (status = 400, description = "リクエストパラメータが不正", body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 409, description = "hook_slug が他 trigger と衝突", body = ErrorResponse),
        (status = 422, description = "リクエストボディのパースに失敗", body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn update_trigger(
    State(state): State<AppState>,
    JsonPath(trigger_id): JsonPath<Uuid>,
    JsonBody(payload): JsonBody<UpdateTriggerRequest>,
) -> Result<Json<trigger::Model>, AppError> {
    let current = find_trigger_or_404(&state.db, trigger_id).await?;
    let mut active = current.clone().into_active_model();

    if let Some(schedule) = payload.schedule {
        if current.kind != TriggerKind::Cron.as_str() {
            return Err(AppError::Validation(
                "schedule can only be set when kind=cron".into(),
            ));
        }
        let trimmed = schedule.trim().to_string();
        if trimmed.is_empty() {
            return Err(AppError::Validation("schedule must not be empty".into()));
        }
        active.schedule = Set(Some(trimmed));
    }
    if let Some(hook_slug) = payload.hook_slug {
        if current.kind != TriggerKind::Hook.as_str() {
            return Err(AppError::Validation(
                "hook_slug can only be set when kind=hook".into(),
            ));
        }
        let trimmed = hook_slug.trim().to_string();
        if trimmed.is_empty() {
            return Err(AppError::Validation("hook_slug must not be empty".into()));
        }
        active.hook_slug = Set(Some(trimmed));
    }
    if let Some(event_match) = payload.event_match {
        active.event_match = Set(Some(event_match));
    }
    if let Some(prompt_template) = payload.prompt_template {
        active.prompt_template = Set(validate_template(&prompt_template)?);
    }
    if let Some(enabled) = payload.enabled {
        active.enabled = Set(enabled);
    }
    active.updated_at = Set(chrono::Utc::now().fixed_offset());

    let updated = active.update(&state.db).await?;
    Ok(Json(updated))
}

/// trigger 削除
#[utoipa::path(
    delete,
    path = "/api/triggers/{trigger_id}",
    tag = "triggers",
    params(("trigger_id" = Uuid, Path, description = "trigger ID")),
    responses(
        (status = 204),
        (status = 400, description = "リクエストパラメータが不正", body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn delete_trigger(
    State(state): State<AppState>,
    JsonPath(trigger_id): JsonPath<Uuid>,
) -> Result<StatusCode, AppError> {
    let result = trigger::Entity::delete_by_id(trigger_id)
        .exec(&state.db)
        .await?;
    if result.rows_affected == 0 {
        return Err(AppError::NotFound(format!(
            "trigger {trigger_id} not found"
        )));
    }
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;
    use serde_json::{Value, json};
    use sqlx::PgPool;

    use crate::testing::create_test_server;

    async fn create_strategy(server: &axum_test::TestServer, name: &str) -> String {
        let res = server
            .post("/api/strategies")
            .json(&json!({ "name": name }))
            .await;
        res.assert_status(StatusCode::CREATED);
        res.json::<Value>()["id"].as_str().unwrap().to_string()
    }

    /// trigger response の時刻系フィールドを placeholder に正規化する。
    /// 全フィールドを 1 つの literal と equality で比較するためのヘルパ。
    /// `trigger_id` は呼び出し側が事前生成 ID を知っているケースとそうでないケースの両方が
    /// あるため、呼び出し側で `preserve_trigger_id=false` を指定したときだけ placeholder 化する。
    fn normalize_trigger(mut value: Value, preserve_trigger_id: bool) -> Value {
        if !preserve_trigger_id && let Some(v) = value.get_mut("trigger_id") {
            *v = Value::String("<trigger_id>".into());
        }
        for key in ["created_at", "updated_at"] {
            if let Some(v) = value.get_mut(key) {
                *v = Value::String(format!("<{key}>"));
            }
        }
        if let Some(v) = value.get_mut("last_fired_at")
            && !v.is_null()
        {
            *v = Value::String("<last_fired_at>".into());
        }
        value
    }

    #[sqlx::test(migrations = false)]
    async fn create_cron_trigger_succeeds(pool: PgPool) {
        let server = create_test_server(pool).await;
        let sid = create_strategy(&server, "s").await;
        let res = server
            .post(&format!("/api/strategies/{sid}/triggers"))
            .json(&json!({
                "kind": "cron",
                "schedule": "0 9 * * 1-5",
                "prompt_template": "morning briefing for {{strategy.name}}",
            }))
            .await;
        res.assert_status(StatusCode::CREATED);
        assert_eq!(
            normalize_trigger(res.json(), false),
            json!({
                "trigger_id": "<trigger_id>",
                "strategy_id": sid,
                "kind": "cron",
                "schedule": "0 9 * * 1-5",
                "hook_slug": null,
                "event_match": null,
                "prompt_template": "morning briefing for {{strategy.name}}",
                "enabled": true,
                "last_fired_at": null,
                "created_at": "<created_at>",
                "updated_at": "<updated_at>",
            }),
        );
    }

    #[sqlx::test(migrations = false)]
    async fn create_hook_trigger_succeeds(pool: PgPool) {
        let server = create_test_server(pool).await;
        let sid = create_strategy(&server, "s").await;
        let res = server
            .post(&format!("/api/strategies/{sid}/triggers"))
            .json(&json!({
                "kind": "hook",
                "hook_slug": "tv-alert",
                "event_match": {"event": {"eq": "fired"}},
                "prompt_template": "alert: {{payload.symbol}}",
            }))
            .await;
        res.assert_status(StatusCode::CREATED);
        assert_eq!(
            normalize_trigger(res.json(), false),
            json!({
                "trigger_id": "<trigger_id>",
                "strategy_id": sid,
                "kind": "hook",
                "schedule": null,
                "hook_slug": "tv-alert",
                "event_match": {"event": {"eq": "fired"}},
                "prompt_template": "alert: {{payload.symbol}}",
                "enabled": true,
                "last_fired_at": null,
                "created_at": "<created_at>",
                "updated_at": "<updated_at>",
            }),
        );
    }

    #[sqlx::test(migrations = false)]
    async fn create_cron_without_schedule_is_400(pool: PgPool) {
        let server = create_test_server(pool).await;
        let sid = create_strategy(&server, "s").await;
        let res = server
            .post(&format!("/api/strategies/{sid}/triggers"))
            .json(&json!({"kind": "cron", "prompt_template": "x"}))
            .await;
        res.assert_status(StatusCode::BAD_REQUEST);
    }

    #[sqlx::test(migrations = false)]
    async fn create_hook_with_schedule_is_400(pool: PgPool) {
        let server = create_test_server(pool).await;
        let sid = create_strategy(&server, "s").await;
        let res = server
            .post(&format!("/api/strategies/{sid}/triggers"))
            .json(&json!({
                "kind": "hook",
                "hook_slug": "x",
                "schedule": "0 * * * *",
                "prompt_template": "x",
            }))
            .await;
        res.assert_status(StatusCode::BAD_REQUEST);
    }

    #[sqlx::test(migrations = false)]
    async fn create_for_missing_strategy_is_404(pool: PgPool) {
        let server = create_test_server(pool).await;
        let res = server
            .post("/api/strategies/00000000-0000-0000-0000-000000000000/triggers")
            .json(&json!({
                "kind": "cron",
                "schedule": "* * * * *",
                "prompt_template": "x",
            }))
            .await;
        res.assert_status(StatusCode::NOT_FOUND);
    }

    #[sqlx::test(migrations = false)]
    async fn duplicate_hook_slug_is_409(pool: PgPool) {
        let server = create_test_server(pool).await;
        let sid = create_strategy(&server, "s").await;
        let body = json!({
            "kind": "hook",
            "hook_slug": "dup",
            "prompt_template": "x",
        });
        server
            .post(&format!("/api/strategies/{sid}/triggers"))
            .json(&body)
            .await
            .assert_status(StatusCode::CREATED);
        let res = server
            .post(&format!("/api/strategies/{sid}/triggers"))
            .json(&body)
            .await;
        res.assert_status(StatusCode::CONFLICT);
    }

    #[sqlx::test(migrations = false)]
    async fn list_filters_by_kind(pool: PgPool) {
        let server = create_test_server(pool).await;
        let sid = create_strategy(&server, "s").await;
        server
            .post(&format!("/api/strategies/{sid}/triggers"))
            .json(&json!({
                "kind": "cron",
                "schedule": "* * * * *",
                "prompt_template": "c",
            }))
            .await
            .assert_status(StatusCode::CREATED);
        server
            .post(&format!("/api/strategies/{sid}/triggers"))
            .json(&json!({
                "kind": "hook",
                "hook_slug": "h",
                "prompt_template": "h",
            }))
            .await
            .assert_status(StatusCode::CREATED);

        let cron_only = server
            .get(&format!("/api/strategies/{sid}/triggers?kind=cron"))
            .await;
        cron_only.assert_status_ok();
        let body: Vec<Value> = cron_only.json();
        let normalized: Vec<Value> = body
            .into_iter()
            .map(|v| normalize_trigger(v, false))
            .collect();
        assert_eq!(
            normalized,
            vec![json!({
                "trigger_id": "<trigger_id>",
                "strategy_id": sid,
                "kind": "cron",
                "schedule": "* * * * *",
                "hook_slug": null,
                "event_match": null,
                "prompt_template": "c",
                "enabled": true,
                "last_fired_at": null,
                "created_at": "<created_at>",
                "updated_at": "<updated_at>",
            })],
        );
    }

    #[sqlx::test(migrations = false)]
    async fn list_scoped_to_owning_strategy(pool: PgPool) {
        let server = create_test_server(pool).await;
        let s1 = create_strategy(&server, "a").await;
        let s2 = create_strategy(&server, "b").await;
        server
            .post(&format!("/api/strategies/{s1}/triggers"))
            .json(&json!({"kind": "cron", "schedule": "* * * * *", "prompt_template": "x"}))
            .await
            .assert_status(StatusCode::CREATED);
        let res = server.get(&format!("/api/strategies/{s2}/triggers")).await;
        res.assert_status_ok();
        assert_eq!(res.json::<Vec<Value>>(), Vec::<Value>::new());
    }

    #[sqlx::test(migrations = false)]
    async fn get_update_delete_round_trip(pool: PgPool) {
        let server = create_test_server(pool).await;
        let sid = create_strategy(&server, "s").await;
        let created: Value = server
            .post(&format!("/api/strategies/{sid}/triggers"))
            .json(&json!({
                "kind": "cron",
                "schedule": "0 9 * * 1-5",
                "prompt_template": "old",
            }))
            .await
            .json();
        let tid = created["trigger_id"].as_str().unwrap().to_string();

        let updated = server
            .put(&format!("/api/triggers/{tid}"))
            .json(&json!({
                "prompt_template": "new",
                "enabled": false,
            }))
            .await;
        updated.assert_status_ok();
        let expected = json!({
            "trigger_id": tid,
            "strategy_id": sid,
            "kind": "cron",
            "schedule": "0 9 * * 1-5",
            "hook_slug": null,
            "event_match": null,
            "prompt_template": "new",
            "enabled": false,
            "last_fired_at": null,
            "created_at": "<created_at>",
            "updated_at": "<updated_at>",
        });
        assert_eq!(normalize_trigger(updated.json(), true), expected);

        let got = server.get(&format!("/api/triggers/{tid}")).await;
        got.assert_status_ok();
        assert_eq!(normalize_trigger(got.json(), true), expected);

        let deleted = server.delete(&format!("/api/triggers/{tid}")).await;
        deleted.assert_status(StatusCode::NO_CONTENT);

        let after = server.get(&format!("/api/triggers/{tid}")).await;
        after.assert_status(StatusCode::NOT_FOUND);
    }

    #[sqlx::test(migrations = false)]
    async fn update_hook_slug_on_cron_trigger_is_400(pool: PgPool) {
        let server = create_test_server(pool).await;
        let sid = create_strategy(&server, "s").await;
        let created: Value = server
            .post(&format!("/api/strategies/{sid}/triggers"))
            .json(&json!({
                "kind": "cron",
                "schedule": "* * * * *",
                "prompt_template": "x",
            }))
            .await
            .json();
        let tid = created["trigger_id"].as_str().unwrap().to_string();
        let res = server
            .put(&format!("/api/triggers/{tid}"))
            .json(&json!({"hook_slug": "x"}))
            .await;
        res.assert_status(StatusCode::BAD_REQUEST);
    }
}
