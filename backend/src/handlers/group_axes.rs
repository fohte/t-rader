//! 分類軸 CRUD の REST handler。

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use core_application::group_axis::{CreateGroupAxisCommand, UpdateGroupAxisCommand};
use serde::Deserialize;
use utoipa::ToSchema;

use crate::AppState;
use crate::error::{AppError, ErrorResponse};
use crate::extractors::{JsonBody, JsonPath};
use crate::models::GroupAxisResponse;

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateGroupAxisRequest {
    pub key: String,
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub sync_source: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct UpdateGroupAxisRequest {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(
        default,
        deserialize_with = "crate::serde_helpers::deserialize_nullable_option"
    )]
    pub sync_source: Option<Option<String>>,
}

/// 分類軸一覧
#[utoipa::path(
    get,
    path = "/api/group-axes",
    tag = "group_axes",
    responses(
        (status = 200, body = Vec<GroupAxisResponse>),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn list_group_axes(
    State(state): State<AppState>,
) -> Result<Json<Vec<GroupAxisResponse>>, AppError> {
    Ok(Json(
        state
            .use_cases
            .group_axes()
            .list()
            .await?
            .into_iter()
            .map(Into::into)
            .collect(),
    ))
}

/// 分類軸を作成
#[utoipa::path(
    post,
    path = "/api/group-axes",
    tag = "group_axes",
    request_body = CreateGroupAxisRequest,
    responses(
        (status = 201, body = GroupAxisResponse),
        (status = 400, body = ErrorResponse),
        (status = 409, description = "key が既存と衝突", body = ErrorResponse),
        (status = 415, description = "リクエストの Content-Type が application/json ではない", body = ErrorResponse),
        (status = 422, description = "リクエストボディのパースに失敗", body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn create_group_axis(
    State(state): State<AppState>,
    JsonBody(payload): JsonBody<CreateGroupAxisRequest>,
) -> Result<(StatusCode, Json<GroupAxisResponse>), AppError> {
    let created = state
        .use_cases
        .group_axes()
        .create(CreateGroupAxisCommand {
            key: payload.key,
            name: payload.name,
            description: payload.description,
            sync_source: payload.sync_source,
        })
        .await?;
    Ok((StatusCode::CREATED, Json(created.into())))
}

/// 分類軸を取得
#[utoipa::path(
    get,
    path = "/api/group-axes/{key}",
    tag = "group_axes",
    params(("key" = String, Path, description = "分類軸 key")),
    responses(
        (status = 200, body = GroupAxisResponse),
        (status = 404, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn get_group_axis(
    State(state): State<AppState>,
    JsonPath(key): JsonPath<String>,
) -> Result<Json<GroupAxisResponse>, AppError> {
    let axis = state.use_cases.group_axes().get(&key).await?;
    Ok(Json(axis.into()))
}

/// 分類軸を部分更新する (key は変更不可)
#[utoipa::path(
    patch,
    path = "/api/group-axes/{key}",
    tag = "group_axes",
    params(("key" = String, Path, description = "分類軸 key")),
    request_body = UpdateGroupAxisRequest,
    responses(
        (status = 200, body = GroupAxisResponse),
        (status = 400, body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 415, description = "リクエストの Content-Type が application/json ではない", body = ErrorResponse),
        (status = 422, description = "リクエストボディのパースに失敗", body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn update_group_axis(
    State(state): State<AppState>,
    JsonPath(key): JsonPath<String>,
    JsonBody(payload): JsonBody<UpdateGroupAxisRequest>,
) -> Result<Json<GroupAxisResponse>, AppError> {
    let axis = state
        .use_cases
        .group_axes()
        .update(
            &key,
            UpdateGroupAxisCommand {
                name: payload.name,
                description: payload.description,
                sync_source: payload.sync_source,
            },
        )
        .await?;
    Ok(Json(axis.into()))
}

/// グループが残っている分類軸は削除しない
#[utoipa::path(
    delete,
    path = "/api/group-axes/{key}",
    tag = "group_axes",
    params(("key" = String, Path, description = "分類軸 key")),
    responses(
        (status = 204),
        (status = 404, body = ErrorResponse),
        (status = 409, description = "分類軸にグループが残っている", body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn delete_group_axis(
    State(state): State<AppState>,
    JsonPath(key): JsonPath<String>,
) -> Result<StatusCode, AppError> {
    state.use_cases.group_axes().delete(&key).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;
    use sea_orm::ActiveValue::Set;
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
    use serde_json::{Value, json};
    use uuid::Uuid;

    use gateway_postgres::entities::{group_axis, stock_group};

    use crate::testing::create_test_server;

    #[backend_test_macros::database_test]
    async fn group_axis_can_be_created_listed_retrieved_and_updated(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let server = create_test_server(db).await;
        let created = server
            .post("/api/group-axes")
            .json(&json!({
                "key": "sample-axis",
                "name": "Sample Axis",
                "description": "A synthetic classification axis",
                "sync_source": "sample-source",
            }))
            .await;
        let created_output = (created.status_code(), created.json::<Value>());

        let fetched = server.get("/api/group-axes/sample-axis").await;
        let fetched_output = (fetched.status_code(), fetched.json::<Value>());

        let listed = server.get("/api/group-axes").await;
        let listed_output = (listed.status_code(), listed.json::<Value>());

        let updated = server
            .patch("/api/group-axes/sample-axis")
            .json(&json!({
                "name": "Updated Axis",
                "sync_source": null,
            }))
            .await;
        let updated_output = (updated.status_code(), updated.json::<Value>());

        let expected_before_update = json!({
            "key": "sample-axis",
            "name": "Sample Axis",
            "description": "A synthetic classification axis",
            "sync_source": "sample-source",
        });
        let expected_after_update = json!({
            "key": "sample-axis",
            "name": "Updated Axis",
            "description": "A synthetic classification axis",
            "sync_source": null,
        });

        assert_eq!(
            (
                created_output,
                fetched_output,
                listed_output,
                updated_output,
            ),
            (
                (StatusCode::CREATED, expected_before_update.clone()),
                (StatusCode::OK, expected_before_update.clone()),
                (StatusCode::OK, json!([expected_before_update])),
                (StatusCode::OK, expected_after_update),
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn deleting_group_axis_with_groups_returns_conflict(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let server = create_test_server(db.clone()).await;
        let created = server
            .post("/api/group-axes")
            .json(&json!({
                "key": "sample-axis",
                "name": "Sample Axis",
                "description": "A synthetic classification axis",
            }))
            .await;
        created.assert_status(StatusCode::CREATED);

        let axis = group_axis::Entity::find()
            .filter(group_axis::Column::Key.eq("sample-axis"))
            .one(&db)
            .await
            .expect("find group axis")
            .expect("group axis exists");
        stock_group::Entity::insert(stock_group::ActiveModel {
            id: Set(Uuid::new_v4()),
            axis_id: Set(axis.id),
            key: Set("sample-group".into()),
            name: Set("Sample Group".into()),
            description: Set(None),
        })
        .exec_without_returning(&db)
        .await
        .expect("insert group");

        let deleted = server.delete("/api/group-axes/sample-axis").await;
        let deleted_output = (deleted.status_code(), deleted.json::<Value>());
        assert_eq!(
            deleted_output,
            (
                StatusCode::CONFLICT,
                json!({
                    "error": "group axis sample-axis cannot be deleted while it contains groups",
                }),
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn deleting_group_axis_without_groups_succeeds(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        let created = server
            .post("/api/group-axes")
            .json(&json!({
                "key": "sample-axis",
                "name": "Sample Axis",
                "description": "A synthetic classification axis",
            }))
            .await;
        created.assert_status(StatusCode::CREATED);

        let deleted = server.delete("/api/group-axes/sample-axis").await;
        let deleted_output = (deleted.status_code(), deleted.text());
        let fetched = server.get("/api/group-axes/sample-axis").await;
        let fetched_output = (fetched.status_code(), fetched.json::<Value>());

        assert_eq!(
            (deleted_output, fetched_output),
            (
                (StatusCode::NO_CONTENT, String::new()),
                (
                    StatusCode::NOT_FOUND,
                    json!({ "error": "group axis sample-axis not found" }),
                ),
            ),
        );
    }
}
