use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use sea_orm::ActiveModelTrait;
use sea_orm::ActiveValue::{NotSet, Set};
use sea_orm::{
    ColumnTrait, EntityTrait, IntoActiveModel, PaginatorTrait, QueryFilter, QueryOrder,
    TransactionTrait,
};
use uuid::Uuid;

use crate::AppState;
use crate::entities::{hypothesis, note};
use crate::error::{AppError, ErrorResponse};
use crate::extractors::{JsonBody, JsonPath};
use crate::models::{CreateHypothesisRequest, UpdateHypothesisRequest};
use crate::services::hypotheses::{DEFAULT_STATUS, ensure_status};
use crate::services::strategies::ensure_strategy_exists;

fn validate_text(field: &str, value: &str) -> Result<String, AppError> {
    let v = value.trim().to_string();
    if v.is_empty() {
        return Err(AppError::Validation(format!("{field} must not be empty")));
    }
    Ok(v)
}

/// `related_note_ids` で渡された note がすべて当該戦略所属であることを検証する。
/// FK を張れないため (migration の comment 参照) アプリ層で同戦略境界を担保する。
async fn ensure_notes_belong_to_strategy<C: sea_orm::ConnectionTrait>(
    conn: &C,
    strategy_id: Uuid,
    ids: &[Uuid],
) -> Result<(), AppError> {
    if ids.is_empty() {
        return Ok(());
    }
    // 重複 UUID で件数比較が偽陽性になるのを避けるため、unique 化してから比較する
    let mut unique_ids: Vec<Uuid> = ids.to_vec();
    unique_ids.sort_unstable();
    unique_ids.dedup();
    let count = note::Entity::find()
        .filter(note::Column::Id.is_in(unique_ids.iter().copied()))
        .filter(note::Column::StrategyId.eq(strategy_id))
        .count(conn)
        .await?;
    if count != unique_ids.len() as u64 {
        return Err(AppError::Validation(
            "related_note_ids contains unknown or cross-strategy note".into(),
        ));
    }
    Ok(())
}

async fn find_hypothesis_for_strategy(
    db: &sea_orm::DatabaseConnection,
    strategy_id: Uuid,
    hypothesis_id: Uuid,
) -> Result<hypothesis::Model, AppError> {
    hypothesis::Entity::find()
        .filter(hypothesis::Column::StrategyId.eq(strategy_id))
        .filter(hypothesis::Column::HypothesisId.eq(hypothesis_id))
        .one(db)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("hypothesis {hypothesis_id} not found")))
}

/// 戦略の仮説一覧 (更新日時の降順)
#[utoipa::path(
    get,
    path = "/api/strategies/{id}/hypotheses",
    tag = "strategies",
    params(("id" = Uuid, Path, description = "戦略 ID")),
    responses(
        (status = 200, body = Vec<hypothesis::Model>),
        (status = 400, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn list_strategy_hypotheses(
    State(state): State<AppState>,
    JsonPath(strategy_id): JsonPath<Uuid>,
) -> Result<Json<Vec<hypothesis::Model>>, AppError> {
    ensure_strategy_exists(&state.db, strategy_id).await?;
    let rows = hypothesis::Entity::find()
        .filter(hypothesis::Column::StrategyId.eq(strategy_id))
        .order_by_desc(hypothesis::Column::UpdatedAt)
        .all(&state.db)
        .await?;
    Ok(Json(rows))
}

/// 仮説を作成する
#[utoipa::path(
    post,
    path = "/api/strategies/{id}/hypotheses",
    tag = "strategies",
    params(("id" = Uuid, Path, description = "戦略 ID")),
    request_body = CreateHypothesisRequest,
    responses(
        (status = 201, body = hypothesis::Model),
        (status = 400, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn create_strategy_hypothesis(
    State(state): State<AppState>,
    JsonPath(strategy_id): JsonPath<Uuid>,
    JsonBody(p): JsonBody<CreateHypothesisRequest>,
) -> Result<(StatusCode, Json<hypothesis::Model>), AppError> {
    let title = validate_text("title", &p.title)?;
    let body = validate_text("body", &p.body)?;
    let status = p.status.unwrap_or_else(|| DEFAULT_STATUS.to_string());
    ensure_status(&status)?;

    let related_note_ids = p.related_note_ids.unwrap_or_default();
    let related_interest_ids = p.related_interest_ids.unwrap_or_default();

    let txn = state.db.begin().await?;
    ensure_strategy_exists(&txn, strategy_id).await?;
    ensure_notes_belong_to_strategy(&txn, strategy_id, &related_note_ids).await?;
    let id = Uuid::new_v4();
    let model = hypothesis::ActiveModel {
        hypothesis_id: Set(id),
        strategy_id: Set(strategy_id),
        title: Set(title),
        body: Set(body),
        status: Set(status),
        related_note_ids: Set(related_note_ids),
        related_interest_ids: Set(related_interest_ids),
        created_at: NotSet,
        updated_at: NotSet,
    };
    let created = hypothesis::Entity::insert(model)
        .exec_with_returning(&txn)
        .await?;
    txn.commit().await?;
    Ok((StatusCode::CREATED, Json(created)))
}

/// 仮説を取得する (戦略境界チェック付き)
#[utoipa::path(
    get,
    path = "/api/strategies/{id}/hypotheses/{hypothesis_id}",
    tag = "strategies",
    params(
        ("id" = Uuid, Path, description = "戦略 ID"),
        ("hypothesis_id" = Uuid, Path, description = "仮説 ID"),
    ),
    responses(
        (status = 200, body = hypothesis::Model),
        (status = 400, body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn get_strategy_hypothesis(
    State(state): State<AppState>,
    JsonPath((strategy_id, hypothesis_id)): JsonPath<(Uuid, Uuid)>,
) -> Result<Json<hypothesis::Model>, AppError> {
    let row = find_hypothesis_for_strategy(&state.db, strategy_id, hypothesis_id).await?;
    Ok(Json(row))
}

/// 仮説を更新する
#[utoipa::path(
    patch,
    path = "/api/strategies/{id}/hypotheses/{hypothesis_id}",
    tag = "strategies",
    params(
        ("id" = Uuid, Path, description = "戦略 ID"),
        ("hypothesis_id" = Uuid, Path, description = "仮説 ID"),
    ),
    request_body = UpdateHypothesisRequest,
    responses(
        (status = 200, body = hypothesis::Model),
        (status = 400, body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn update_strategy_hypothesis(
    State(state): State<AppState>,
    JsonPath((strategy_id, hypothesis_id)): JsonPath<(Uuid, Uuid)>,
    JsonBody(p): JsonBody<UpdateHypothesisRequest>,
) -> Result<Json<hypothesis::Model>, AppError> {
    let current = find_hypothesis_for_strategy(&state.db, strategy_id, hypothesis_id).await?;
    let mut active = current.clone().into_active_model();
    let mut touched = false;
    if let Some(title) = p.title {
        let title = validate_text("title", &title)?;
        active.title = Set(title);
        touched = true;
    }
    if let Some(body) = p.body {
        let body = validate_text("body", &body)?;
        active.body = Set(body);
        touched = true;
    }
    if let Some(status) = p.status {
        ensure_status(&status)?;
        active.status = Set(status);
        touched = true;
    }
    if let Some(ids) = p.related_note_ids {
        ensure_notes_belong_to_strategy(&state.db, strategy_id, &ids).await?;
        active.related_note_ids = Set(ids);
        touched = true;
    }
    if let Some(ids) = p.related_interest_ids {
        active.related_interest_ids = Set(ids);
        touched = true;
    }
    if !touched {
        return Err(AppError::Validation(
            "at least one field must be provided".into(),
        ));
    }
    active.updated_at = Set(chrono::Utc::now().fixed_offset());
    let updated = active.update(&state.db).await?;
    Ok(Json(updated))
}

/// 仮説を削除する
#[utoipa::path(
    delete,
    path = "/api/strategies/{id}/hypotheses/{hypothesis_id}",
    tag = "strategies",
    params(
        ("id" = Uuid, Path, description = "戦略 ID"),
        ("hypothesis_id" = Uuid, Path, description = "仮説 ID"),
    ),
    responses(
        (status = 204),
        (status = 400, body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn delete_strategy_hypothesis(
    State(state): State<AppState>,
    JsonPath((strategy_id, hypothesis_id)): JsonPath<(Uuid, Uuid)>,
) -> Result<StatusCode, AppError> {
    let res = hypothesis::Entity::delete_many()
        .filter(hypothesis::Column::StrategyId.eq(strategy_id))
        .filter(hypothesis::Column::HypothesisId.eq(hypothesis_id))
        .exec(&state.db)
        .await?;
    if res.rows_affected == 0 {
        return Err(AppError::NotFound(format!(
            "hypothesis {hypothesis_id} not found"
        )));
    }
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;
    use sea_orm::ActiveModelTrait;
    use sea_orm::ActiveValue::{NotSet, Set};
    use sea_orm::DatabaseConnection;
    use serde_json::json;
    use sqlx::PgPool;
    use uuid::Uuid;

    use crate::entities::note;
    use crate::testing::{create_test_server_with_db, insert_test_strategy};

    async fn seed_note(db: &DatabaseConnection, strategy_id: Uuid) -> Uuid {
        let id = Uuid::new_v4();
        note::ActiveModel {
            id: Set(id),
            strategy_id: Set(strategy_id),
            title: Set("t".into()),
            body_md: Set("b".into()),
            frontmatter_json: Set(json!({})),
            type_tag: Set(None),
            status: Set("unread".into()),
            trigger: Set(None),
            trigger_label: Set(None),
            created_by_kind: Set("human".into()),
            created_at: NotSet,
            updated_at: NotSet,
        }
        .insert(db)
        .await
        .expect("insert note");
        id
    }

    fn normalize(mut v: serde_json::Value) -> serde_json::Value {
        let o = v.as_object_mut().unwrap();
        o.remove("created_at");
        o.remove("updated_at");
        o.insert("hypothesis_id".into(), json!("<uuid>"));
        v
    }

    #[sqlx::test(migrations = false)]
    async fn create_then_list_get_round_trips(pool: PgPool) {
        let (db, server) = create_test_server_with_db(pool).await;
        let sid = insert_test_strategy(&db, "s").await;

        let created = server
            .post(&format!("/api/strategies/{sid}/hypotheses"))
            .json(&json!({
                "title": "USD/JPY 押し目買い",
                "body": "実体は **下値** 試し",
            }))
            .await;
        created.assert_status(StatusCode::CREATED);
        let body: serde_json::Value = created.json();
        let hid = Uuid::parse_str(body["hypothesis_id"].as_str().unwrap()).unwrap();

        assert_eq!(
            normalize(body),
            json!({
                "hypothesis_id": "<uuid>",
                "strategy_id": sid,
                "title": "USD/JPY 押し目買い",
                "body": "実体は **下値** 試し",
                "status": "unverified",
                "related_note_ids": [],
                "related_interest_ids": [],
            }),
        );

        let got = server
            .get(&format!("/api/strategies/{sid}/hypotheses/{hid}"))
            .await;
        got.assert_status_ok();
        assert_eq!(
            normalize(got.json()),
            json!({
                "hypothesis_id": "<uuid>",
                "strategy_id": sid,
                "title": "USD/JPY 押し目買い",
                "body": "実体は **下値** 試し",
                "status": "unverified",
                "related_note_ids": [],
                "related_interest_ids": [],
            }),
        );

        let list = server
            .get(&format!("/api/strategies/{sid}/hypotheses"))
            .await;
        list.assert_status_ok();
        let arr: Vec<serde_json::Value> = list.json();
        let normalized: Vec<_> = arr.into_iter().map(normalize).collect();
        assert_eq!(
            normalized,
            vec![json!({
                "hypothesis_id": "<uuid>",
                "strategy_id": sid,
                "title": "USD/JPY 押し目買い",
                "body": "実体は **下値** 試し",
                "status": "unverified",
                "related_note_ids": [],
                "related_interest_ids": [],
            })],
        );
    }

    #[sqlx::test(migrations = false)]
    async fn create_with_status_and_related_ids(pool: PgPool) {
        let (db, server) = create_test_server_with_db(pool).await;
        let sid = insert_test_strategy(&db, "s").await;
        let n1 = seed_note(&db, sid).await;
        let i1 = Uuid::new_v4();

        let created = server
            .post(&format!("/api/strategies/{sid}/hypotheses"))
            .json(&json!({
                "title": "t",
                "body": "b",
                "status": "supported",
                "related_note_ids": [n1],
                "related_interest_ids": [i1],
            }))
            .await;
        created.assert_status(StatusCode::CREATED);
        assert_eq!(
            normalize(created.json()),
            json!({
                "hypothesis_id": "<uuid>",
                "strategy_id": sid,
                "title": "t",
                "body": "b",
                "status": "supported",
                "related_note_ids": [n1],
                "related_interest_ids": [i1],
            }),
        );
    }

    #[sqlx::test(migrations = false)]
    async fn create_accepts_duplicate_related_note_ids(pool: PgPool) {
        let (db, server) = create_test_server_with_db(pool).await;
        let sid = insert_test_strategy(&db, "s").await;
        let n1 = seed_note(&db, sid).await;
        // 同一 note を重複指定しても、unique 化後の存在チェックを通過する
        let res = server
            .post(&format!("/api/strategies/{sid}/hypotheses"))
            .json(&json!({
                "title": "t",
                "body": "b",
                "related_note_ids": [n1, n1],
            }))
            .await;
        res.assert_status(StatusCode::CREATED);
    }

    #[sqlx::test(migrations = false)]
    async fn create_rejects_unknown_related_note(pool: PgPool) {
        let (db, server) = create_test_server_with_db(pool).await;
        let sid = insert_test_strategy(&db, "s").await;
        let res = server
            .post(&format!("/api/strategies/{sid}/hypotheses"))
            .json(&json!({
                "title": "t",
                "body": "b",
                "related_note_ids": [Uuid::new_v4()],
            }))
            .await;
        res.assert_status(StatusCode::BAD_REQUEST);
    }

    #[sqlx::test(migrations = false)]
    async fn create_rejects_cross_strategy_related_note(pool: PgPool) {
        let (db, server) = create_test_server_with_db(pool).await;
        let a = insert_test_strategy(&db, "a").await;
        let b = insert_test_strategy(&db, "b").await;
        let n_b = seed_note(&db, b).await;
        let res = server
            .post(&format!("/api/strategies/{a}/hypotheses"))
            .json(&json!({
                "title": "t",
                "body": "b",
                "related_note_ids": [n_b],
            }))
            .await;
        res.assert_status(StatusCode::BAD_REQUEST);
    }

    // 入力 validation の各 reject ケースを 1 関数にまとめる。
    // rstest #[case] は sqlx::test の pool 注入と組み合わせ難く、ケース数が少ないため for ループで列挙する。
    #[sqlx::test(migrations = false)]
    async fn create_rejects_invalid_inputs(pool: PgPool) {
        let (db, server) = create_test_server_with_db(pool).await;
        let sid = insert_test_strategy(&db, "s").await;

        for (label, body) in [
            ("empty_title", json!({"title": "", "body": "b"})),
            ("empty_body", json!({"title": "t", "body": ""})),
            (
                "invalid_status",
                json!({"title": "t", "body": "b", "status": "bogus"}),
            ),
        ] {
            let r = server
                .post(&format!("/api/strategies/{sid}/hypotheses"))
                .json(&body)
                .await;
            assert_eq!(
                r.status_code(),
                StatusCode::BAD_REQUEST,
                "case {label} did not return 400",
            );
        }
    }

    #[sqlx::test(migrations = false)]
    async fn create_for_unknown_strategy_returns_400(pool: PgPool) {
        let (_db, server) = create_test_server_with_db(pool).await;
        let res = server
            .post("/api/strategies/00000000-0000-0000-0000-000000000000/hypotheses")
            .json(&json!({"title": "t", "body": "b"}))
            .await;
        res.assert_status(StatusCode::BAD_REQUEST);
    }

    #[sqlx::test(migrations = false)]
    async fn update_changes_fields(pool: PgPool) {
        let (db, server) = create_test_server_with_db(pool).await;
        let sid = insert_test_strategy(&db, "s").await;
        let created = server
            .post(&format!("/api/strategies/{sid}/hypotheses"))
            .json(&json!({"title": "t", "body": "b"}))
            .await;
        created.assert_status(StatusCode::CREATED);
        let hid = Uuid::parse_str(
            created.json::<serde_json::Value>()["hypothesis_id"]
                .as_str()
                .unwrap(),
        )
        .unwrap();

        let res = server
            .patch(&format!("/api/strategies/{sid}/hypotheses/{hid}"))
            .json(&json!({"status": "refuted", "body": "new"}))
            .await;
        res.assert_status_ok();
        assert_eq!(
            normalize(res.json()),
            json!({
                "hypothesis_id": "<uuid>",
                "strategy_id": sid,
                "title": "t",
                "body": "new",
                "status": "refuted",
                "related_note_ids": [],
                "related_interest_ids": [],
            }),
        );
    }

    #[sqlx::test(migrations = false)]
    async fn update_empty_body_rejected(pool: PgPool) {
        let (db, server) = create_test_server_with_db(pool).await;
        let sid = insert_test_strategy(&db, "s").await;
        let created = server
            .post(&format!("/api/strategies/{sid}/hypotheses"))
            .json(&json!({"title": "t", "body": "b"}))
            .await;
        let hid = Uuid::parse_str(
            created.json::<serde_json::Value>()["hypothesis_id"]
                .as_str()
                .unwrap(),
        )
        .unwrap();
        let res = server
            .patch(&format!("/api/strategies/{sid}/hypotheses/{hid}"))
            .json(&json!({}))
            .await;
        res.assert_status(StatusCode::BAD_REQUEST);
    }

    #[sqlx::test(migrations = false)]
    async fn cross_strategy_get_update_delete_return_404(pool: PgPool) {
        let (db, server) = create_test_server_with_db(pool).await;
        let a = insert_test_strategy(&db, "a").await;
        let b = insert_test_strategy(&db, "b").await;
        let created = server
            .post(&format!("/api/strategies/{a}/hypotheses"))
            .json(&json!({"title": "t", "body": "b"}))
            .await;
        created.assert_status(StatusCode::CREATED);
        let hid = Uuid::parse_str(
            created.json::<serde_json::Value>()["hypothesis_id"]
                .as_str()
                .unwrap(),
        )
        .unwrap();

        for path in [format!("/api/strategies/{b}/hypotheses/{hid}")] {
            let g = server.get(&path).await;
            g.assert_status(StatusCode::NOT_FOUND);
            let p = server
                .patch(&path)
                .json(&json!({"status": "obsolete"}))
                .await;
            p.assert_status(StatusCode::NOT_FOUND);
            let d = server.delete(&path).await;
            d.assert_status(StatusCode::NOT_FOUND);
        }
    }

    #[sqlx::test(migrations = false)]
    async fn delete_existing_returns_204(pool: PgPool) {
        let (db, server) = create_test_server_with_db(pool).await;
        let sid = insert_test_strategy(&db, "s").await;
        let created = server
            .post(&format!("/api/strategies/{sid}/hypotheses"))
            .json(&json!({"title": "t", "body": "b"}))
            .await;
        let hid = Uuid::parse_str(
            created.json::<serde_json::Value>()["hypothesis_id"]
                .as_str()
                .unwrap(),
        )
        .unwrap();
        let res = server
            .delete(&format!("/api/strategies/{sid}/hypotheses/{hid}"))
            .await;
        res.assert_status(StatusCode::NO_CONTENT);

        let again = server
            .get(&format!("/api/strategies/{sid}/hypotheses/{hid}"))
            .await;
        again.assert_status(StatusCode::NOT_FOUND);
    }

    #[sqlx::test(migrations = false)]
    async fn cascade_delete_when_strategy_deleted(pool: PgPool) {
        let (db, server) = create_test_server_with_db(pool).await;
        let sid = insert_test_strategy(&db, "s").await;
        let created = server
            .post(&format!("/api/strategies/{sid}/hypotheses"))
            .json(&json!({"title": "t", "body": "b"}))
            .await;
        let hid = Uuid::parse_str(
            created.json::<serde_json::Value>()["hypothesis_id"]
                .as_str()
                .unwrap(),
        )
        .unwrap();

        use sea_orm::EntityTrait;
        crate::entities::strategy::Entity::delete_by_id(sid)
            .exec(&db)
            .await
            .expect("delete strategy");

        let row = crate::entities::hypothesis::Entity::find_by_id(hid)
            .one(&db)
            .await
            .expect("query");
        assert!(row.is_none());
    }
}
