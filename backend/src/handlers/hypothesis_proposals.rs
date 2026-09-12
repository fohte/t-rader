use axum::Json;
use axum::extract::State;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder};
use serde::Deserialize;
use utoipa::IntoParams;
use uuid::Uuid;

use crate::AppState;
use crate::entities::hypothesis_proposal;
use crate::error::{AppError, ErrorResponse};
use crate::extractors::{JsonBody, JsonPath, JsonQuery};
use crate::models::{ApproveHypothesisProposalResponse, ReviewHypothesisProposalRequest};
use crate::services::hypotheses::find_hypothesis_or_404;
use crate::services::hypothesis_proposals;

async fn find_proposal_or_404(
    db: &DatabaseConnection,
    id: Uuid,
) -> Result<hypothesis_proposal::Model, AppError> {
    hypothesis_proposal::Entity::find_by_id(id)
        .one(db)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("hypothesis_proposal {id} not found")))
}

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ListHypothesisProposalsQuery {
    pub hypothesis_id: Option<Uuid>,
    pub status: Option<String>,
}

/// 仮説への変更提案一覧 (作成日時降順)。`hypothesis_id`/`status` で任意に絞り込める。
#[utoipa::path(
    get,
    path = "/api/hypothesis-proposals",
    tag = "hypothesis_proposals",
    params(ListHypothesisProposalsQuery),
    responses(
        (status = 200, body = Vec<hypothesis_proposal::Model>),
        (status = 400, description = "リクエストパラメータが不正", body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn list_hypothesis_proposals(
    State(state): State<AppState>,
    JsonQuery(params): JsonQuery<ListHypothesisProposalsQuery>,
) -> Result<Json<Vec<hypothesis_proposal::Model>>, AppError> {
    let mut q =
        hypothesis_proposal::Entity::find().order_by_desc(hypothesis_proposal::Column::CreatedAt);
    if let Some(hid) = params.hypothesis_id {
        q = q.filter(hypothesis_proposal::Column::HypothesisId.eq(hid));
    }
    if let Some(status) = params.status.as_deref().filter(|s| !s.is_empty()) {
        q = q.filter(hypothesis_proposal::Column::Status.eq(status));
    }
    Ok(Json(q.all(&state.db).await?))
}

/// 特定の仮説への変更提案一覧 (作成日時降順)
#[utoipa::path(
    get,
    path = "/api/hypotheses/{id}/proposals",
    tag = "hypotheses",
    params(("id" = Uuid, Path, description = "仮説 ID")),
    responses(
        (status = 200, body = Vec<hypothesis_proposal::Model>),
        (status = 400, body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn list_proposals_for_hypothesis(
    State(state): State<AppState>,
    JsonPath(hypothesis_id): JsonPath<Uuid>,
) -> Result<Json<Vec<hypothesis_proposal::Model>>, AppError> {
    find_hypothesis_or_404(&state.db, hypothesis_id).await?;
    let rows = hypothesis_proposal::Entity::find()
        .filter(hypothesis_proposal::Column::HypothesisId.eq(hypothesis_id))
        .order_by_desc(hypothesis_proposal::Column::CreatedAt)
        .all(&state.db)
        .await?;
    Ok(Json(rows))
}

/// 提案を取得する
#[utoipa::path(
    get,
    path = "/api/hypothesis-proposals/{id}",
    tag = "hypothesis_proposals",
    params(("id" = Uuid, Path, description = "提案 ID")),
    responses(
        (status = 200, body = hypothesis_proposal::Model),
        (status = 400, body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn get_hypothesis_proposal(
    State(state): State<AppState>,
    JsonPath(id): JsonPath<Uuid>,
) -> Result<Json<hypothesis_proposal::Model>, AppError> {
    Ok(Json(find_proposal_or_404(&state.db, id).await?))
}

/// 提案を承認する。`pending` の場合のみ指定されたフィールドを仮説本体に反映する。
/// 既に `approved` の場合は再適用せず現在値を返し (200)、`rejected` の場合は 409 を返す。
#[utoipa::path(
    post,
    path = "/api/hypothesis-proposals/{id}/approve",
    tag = "hypothesis_proposals",
    params(("id" = Uuid, Path, description = "提案 ID")),
    request_body = ReviewHypothesisProposalRequest,
    responses(
        (status = 200, body = ApproveHypothesisProposalResponse),
        (status = 400, body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 409, description = "既に却下済み", body = ErrorResponse),
        (status = 415, description = "Content-Type ヘッダが application/json ではない", body = ErrorResponse),
        (status = 422, description = "リクエストボディのパースに失敗", body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn approve_hypothesis_proposal(
    State(state): State<AppState>,
    JsonPath(id): JsonPath<Uuid>,
    JsonBody(payload): JsonBody<ReviewHypothesisProposalRequest>,
) -> Result<Json<ApproveHypothesisProposalResponse>, AppError> {
    let (proposal, hypothesis) =
        hypothesis_proposals::approve_proposal(&state.db, id, payload.review_note).await?;
    Ok(Json(ApproveHypothesisProposalResponse {
        proposal,
        hypothesis,
    }))
}

/// 提案を却下する。仮説本体には反映されない。
#[utoipa::path(
    post,
    path = "/api/hypothesis-proposals/{id}/reject",
    tag = "hypothesis_proposals",
    params(("id" = Uuid, Path, description = "提案 ID")),
    request_body = ReviewHypothesisProposalRequest,
    responses(
        (status = 200, body = hypothesis_proposal::Model),
        (status = 400, body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 409, description = "既に承認済み", body = ErrorResponse),
        (status = 415, description = "Content-Type ヘッダが application/json ではない", body = ErrorResponse),
        (status = 422, description = "リクエストボディのパースに失敗", body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn reject_hypothesis_proposal(
    State(state): State<AppState>,
    JsonPath(id): JsonPath<Uuid>,
    JsonBody(payload): JsonBody<ReviewHypothesisProposalRequest>,
) -> Result<Json<hypothesis_proposal::Model>, AppError> {
    Ok(Json(
        hypothesis_proposals::reject_proposal(&state.db, id, payload.review_note).await?,
    ))
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;
    use chrono::{DateTime, FixedOffset, TimeZone, Utc};
    use sea_orm::ActiveModelTrait;
    use sea_orm::ActiveValue::Set;
    use sea_orm::{EntityTrait, IntoActiveModel};
    use serde_json::{Value, json};
    use sqlx::PgPool;
    use uuid::Uuid;

    use crate::entities::hypothesis;
    use crate::testing::{
        create_test_server_with_db, insert_test_hypothesis, insert_test_hypothesis_proposal,
        insert_test_strategy,
    };

    fn at(minute: i64) -> DateTime<FixedOffset> {
        (Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap() + chrono::Duration::minutes(minute))
            .fixed_offset()
    }

    fn normalize_proposal(mut v: Value) -> Value {
        let o = v.as_object_mut().expect("proposal is an object");
        o.insert("created_at".into(), json!("<dyn>"));
        let reviewed_at = if o["reviewed_at"].is_null() {
            Value::Null
        } else {
            json!("<dyn>")
        };
        o.insert("reviewed_at".into(), reviewed_at);
        v
    }

    fn normalize_hypothesis(mut v: Value) -> Value {
        let o = v.as_object_mut().expect("hypothesis is an object");
        o.insert("created_at".into(), json!("<dyn>"));
        o.insert("updated_at".into(), json!("<dyn>"));
        v
    }

    fn normalize_approve_response(mut v: Value) -> Value {
        let o = v.as_object_mut().expect("response is an object");
        let proposal = normalize_proposal(o.remove("proposal").expect("proposal field"));
        let hypothesis = normalize_hypothesis(o.remove("hypothesis").expect("hypothesis field"));
        o.insert("proposal".into(), proposal);
        o.insert("hypothesis".into(), hypothesis);
        v
    }

    #[sqlx::test(migrations = false)]
    async fn list_hypothesis_proposals_filters_by_hypothesis_id_and_status(pool: PgPool) {
        let (db, server) = create_test_server_with_db(pool).await;
        let sid = insert_test_strategy(&db, "s").await;
        let h1 = insert_test_hypothesis(&db, Some(sid), "h1", "b1", "unverified").await;
        let h2 = insert_test_hypothesis(&db, Some(sid), "h2", "b2", "unverified").await;
        let p1 = insert_test_hypothesis_proposal(
            &db,
            h1,
            Some("new1"),
            None,
            None,
            "r1",
            "pending",
            at(0),
        )
        .await;
        let p2 = insert_test_hypothesis_proposal(
            &db,
            h1,
            Some("new2"),
            None,
            None,
            "r2",
            "approved",
            at(1),
        )
        .await;
        let p3 = insert_test_hypothesis_proposal(
            &db,
            h2,
            Some("new3"),
            None,
            None,
            "r3",
            "pending",
            at(2),
        )
        .await;

        let ids_of = |res: axum_test::TestResponse| -> Vec<Uuid> {
            res.json::<Vec<Value>>()
                .iter()
                .map(|v| Uuid::parse_str(v["id"].as_str().expect("id")).expect("uuid"))
                .collect()
        };

        let all = server.get("/api/hypothesis-proposals").await;
        all.assert_status_ok();
        assert_eq!(ids_of(all), vec![p3, p2, p1]);

        let by_hypothesis = server
            .get(&format!("/api/hypothesis-proposals?hypothesis_id={h1}"))
            .await;
        by_hypothesis.assert_status_ok();
        assert_eq!(ids_of(by_hypothesis), vec![p2, p1]);

        let by_status = server
            .get("/api/hypothesis-proposals?status=approved")
            .await;
        by_status.assert_status_ok();
        assert_eq!(ids_of(by_status), vec![p2]);
    }

    #[sqlx::test(migrations = false)]
    async fn list_proposals_for_hypothesis_returns_scoped_list_and_404_for_unknown(pool: PgPool) {
        let (db, server) = create_test_server_with_db(pool).await;
        let sid = insert_test_strategy(&db, "s").await;
        let h1 = insert_test_hypothesis(&db, Some(sid), "h1", "b1", "unverified").await;
        let h2 = insert_test_hypothesis(&db, Some(sid), "h2", "b2", "unverified").await;
        insert_test_hypothesis_proposal(&db, h1, Some("new1"), None, None, "r1", "pending", at(0))
            .await;
        let p2 = insert_test_hypothesis_proposal(
            &db,
            h2,
            Some("new2"),
            None,
            None,
            "r2",
            "pending",
            at(1),
        )
        .await;

        let scoped = server.get(&format!("/api/hypotheses/{h2}/proposals")).await;
        scoped.assert_status_ok();
        let ids: Vec<Uuid> = scoped
            .json::<Vec<Value>>()
            .iter()
            .map(|v| Uuid::parse_str(v["id"].as_str().expect("id")).expect("uuid"))
            .collect();
        assert_eq!(ids, vec![p2]);

        let unknown = server
            .get(&format!("/api/hypotheses/{}/proposals", Uuid::new_v4()))
            .await;
        unknown.assert_status(StatusCode::NOT_FOUND);
    }

    #[sqlx::test(migrations = false)]
    async fn get_returns_404_for_unknown_id(pool: PgPool) {
        let (_db, server) = create_test_server_with_db(pool).await;
        let res = server
            .get(&format!("/api/hypothesis-proposals/{}", Uuid::new_v4()))
            .await;
        res.assert_status(StatusCode::NOT_FOUND);
    }

    #[sqlx::test(migrations = false)]
    async fn approve_applies_only_specified_fields(pool: PgPool) {
        let (db, server) = create_test_server_with_db(pool).await;
        let sid = insert_test_strategy(&db, "s").await;
        let hid =
            insert_test_hypothesis(&db, Some(sid), "元タイトル", "元本文", "unverified").await;
        let pid = insert_test_hypothesis_proposal(
            &db,
            hid,
            Some("新タイトル"),
            None,
            Some("supported"),
            "根拠",
            "pending",
            at(0),
        )
        .await;

        let res = server
            .post(&format!("/api/hypothesis-proposals/{pid}/approve"))
            .json(&json!({ "review_note": "良さそう" }))
            .await;
        res.assert_status_ok();
        assert_eq!(
            normalize_approve_response(res.json()),
            json!({
                "proposal": {
                    "id": pid,
                    "hypothesis_id": hid,
                    "proposed_title": "新タイトル",
                    "proposed_body": null,
                    "proposed_status": "supported",
                    "rationale": "根拠",
                    "status": "approved",
                    "review_note": "良さそう",
                    "created_at": "<dyn>",
                    "reviewed_at": "<dyn>",
                },
                "hypothesis": {
                    "hypothesis_id": hid,
                    "strategy_id": sid,
                    "title": "新タイトル",
                    "body": "元本文",
                    "status": "supported",
                    "related_note_ids": [],
                    "related_interest_ids": [],
                    "created_at": "<dyn>",
                    "updated_at": "<dyn>",
                },
            }),
        );
    }

    #[sqlx::test(migrations = false)]
    async fn approve_is_idempotent_and_does_not_reapply(pool: PgPool) {
        let (db, server) = create_test_server_with_db(pool).await;
        let sid = insert_test_strategy(&db, "s").await;
        let hid =
            insert_test_hypothesis(&db, Some(sid), "元タイトル", "元本文", "unverified").await;
        let pid = insert_test_hypothesis_proposal(
            &db,
            hid,
            Some("新タイトル"),
            None,
            None,
            "根拠",
            "pending",
            at(0),
        )
        .await;

        server
            .post(&format!("/api/hypothesis-proposals/{pid}/approve"))
            .json(&json!({}))
            .await
            .assert_status_ok();

        // 承認後に人間が直接編集しても、2 回目の approve は再適用しないためこの変更は保持される
        let mut active = hypothesis::Entity::find_by_id(hid)
            .one(&db)
            .await
            .expect("query")
            .expect("hypothesis exists")
            .into_active_model();
        active.title = Set("人間による再編集".into());
        active.update(&db).await.expect("update hypothesis");

        let res = server
            .post(&format!("/api/hypothesis-proposals/{pid}/approve"))
            .json(&json!({}))
            .await;
        res.assert_status_ok();
        assert_eq!(
            normalize_approve_response(res.json()),
            json!({
                "proposal": {
                    "id": pid,
                    "hypothesis_id": hid,
                    "proposed_title": "新タイトル",
                    "proposed_body": null,
                    "proposed_status": null,
                    "rationale": "根拠",
                    "status": "approved",
                    "review_note": null,
                    "created_at": "<dyn>",
                    "reviewed_at": "<dyn>",
                },
                "hypothesis": {
                    "hypothesis_id": hid,
                    "strategy_id": sid,
                    "title": "人間による再編集",
                    "body": "元本文",
                    "status": "unverified",
                    "related_note_ids": [],
                    "related_interest_ids": [],
                    "created_at": "<dyn>",
                    "updated_at": "<dyn>",
                },
            }),
        );
    }

    #[sqlx::test(migrations = false)]
    async fn reject_then_approve_returns_409(pool: PgPool) {
        let (db, server) = create_test_server_with_db(pool).await;
        let sid = insert_test_strategy(&db, "s").await;
        let hid = insert_test_hypothesis(&db, Some(sid), "t", "b", "unverified").await;
        let pid = insert_test_hypothesis_proposal(
            &db,
            hid,
            Some("new"),
            None,
            None,
            "根拠",
            "pending",
            at(0),
        )
        .await;

        server
            .post(&format!("/api/hypothesis-proposals/{pid}/reject"))
            .json(&json!({}))
            .await
            .assert_status_ok();

        let res = server
            .post(&format!("/api/hypothesis-proposals/{pid}/approve"))
            .json(&json!({}))
            .await;
        res.assert_status(StatusCode::CONFLICT);
    }

    #[sqlx::test(migrations = false)]
    async fn reject_applies_and_does_not_touch_hypothesis(pool: PgPool) {
        let (db, server) = create_test_server_with_db(pool).await;
        let sid = insert_test_strategy(&db, "s").await;
        let hid = insert_test_hypothesis(&db, Some(sid), "t", "b", "unverified").await;
        let pid = insert_test_hypothesis_proposal(
            &db,
            hid,
            Some("new"),
            None,
            None,
            "根拠",
            "pending",
            at(0),
        )
        .await;

        let res = server
            .post(&format!("/api/hypothesis-proposals/{pid}/reject"))
            .json(&json!({ "review_note": "根拠不足" }))
            .await;
        res.assert_status_ok();
        assert_eq!(
            normalize_proposal(res.json()),
            json!({
                "id": pid,
                "hypothesis_id": hid,
                "proposed_title": "new",
                "proposed_body": null,
                "proposed_status": null,
                "rationale": "根拠",
                "status": "rejected",
                "review_note": "根拠不足",
                "created_at": "<dyn>",
                "reviewed_at": "<dyn>",
            }),
        );

        let current = hypothesis::Entity::find_by_id(hid)
            .one(&db)
            .await
            .expect("query")
            .expect("hypothesis exists");
        assert_eq!(current.title, "t");
    }

    #[sqlx::test(migrations = false)]
    async fn reject_is_idempotent(pool: PgPool) {
        let (db, server) = create_test_server_with_db(pool).await;
        let sid = insert_test_strategy(&db, "s").await;
        let hid = insert_test_hypothesis(&db, Some(sid), "t", "b", "unverified").await;
        let pid = insert_test_hypothesis_proposal(
            &db,
            hid,
            Some("new"),
            None,
            None,
            "根拠",
            "pending",
            at(0),
        )
        .await;

        let first = server
            .post(&format!("/api/hypothesis-proposals/{pid}/reject"))
            .json(&json!({ "review_note": "最初の note" }))
            .await;
        first.assert_status_ok();

        let second = server
            .post(&format!("/api/hypothesis-proposals/{pid}/reject"))
            .json(&json!({ "review_note": "2 回目の note" }))
            .await;
        second.assert_status_ok();

        assert_eq!(
            normalize_proposal(second.json()),
            normalize_proposal(first.json()),
        );
    }

    #[sqlx::test(migrations = false)]
    async fn approve_then_reject_returns_409(pool: PgPool) {
        let (db, server) = create_test_server_with_db(pool).await;
        let sid = insert_test_strategy(&db, "s").await;
        let hid = insert_test_hypothesis(&db, Some(sid), "t", "b", "unverified").await;
        let pid = insert_test_hypothesis_proposal(
            &db,
            hid,
            Some("new"),
            None,
            None,
            "根拠",
            "pending",
            at(0),
        )
        .await;

        server
            .post(&format!("/api/hypothesis-proposals/{pid}/approve"))
            .json(&json!({}))
            .await
            .assert_status_ok();

        let res = server
            .post(&format!("/api/hypothesis-proposals/{pid}/reject"))
            .json(&json!({}))
            .await;
        res.assert_status(StatusCode::CONFLICT);
    }

    #[sqlx::test(migrations = false)]
    async fn approve_and_reject_return_404_for_unknown_id(pool: PgPool) {
        let (_db, server) = create_test_server_with_db(pool).await;
        let unknown = Uuid::new_v4();

        for path in [
            format!("/api/hypothesis-proposals/{unknown}/approve"),
            format!("/api/hypothesis-proposals/{unknown}/reject"),
        ] {
            let res = server.post(&path).json(&json!({})).await;
            assert_eq!(
                res.status_code(),
                StatusCode::NOT_FOUND,
                "path {path} did not return 404",
            );
        }
    }
}
