use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use sea_orm::ActiveValue::Set;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
use uuid::Uuid;

use crate::AppState;
use crate::entities::{hypothesis, note, note_hypothesis};
use crate::error::{AppError, ErrorResponse};
use crate::extractors::{JsonBody, JsonPath};
use crate::models::CreateNoteHypothesisRequest;

async fn find_note_or_404(
    db: &sea_orm::DatabaseConnection,
    note_id: Uuid,
) -> Result<note::Model, AppError> {
    note::Entity::find_by_id(note_id)
        .one(db)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("note {note_id} not found")))
}

/// ノートが依拠した仮説一覧 (リンク作成順)。各行はリンク時点の仮説内容を snapshot している。
#[utoipa::path(
    get,
    path = "/api/notes/{id}/hypotheses",
    tag = "notes",
    params(("id" = Uuid, Path, description = "ノート ID")),
    responses(
        (status = 200, body = Vec<note_hypothesis::Model>),
        (status = 400, body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn list_note_hypotheses(
    State(state): State<AppState>,
    JsonPath(note_id): JsonPath<Uuid>,
) -> Result<Json<Vec<note_hypothesis::Model>>, AppError> {
    find_note_or_404(&state.db, note_id).await?;

    let links = note_hypothesis::Entity::find()
        .filter(note_hypothesis::Column::NoteId.eq(note_id))
        .order_by_asc(note_hypothesis::Column::CreatedAt)
        .all(&state.db)
        .await?;
    Ok(Json(links))
}

/// ノートに仮説を紐付ける。仮説の title/body/status はこの時点の内容を snapshot し、
/// 以後の仮説編集では書き換わらない (過去のノートが依拠した根拠を固定するため)。
#[utoipa::path(
    post,
    path = "/api/notes/{id}/hypotheses",
    tag = "notes",
    params(("id" = Uuid, Path, description = "ノート ID")),
    request_body = CreateNoteHypothesisRequest,
    responses(
        (status = 201, body = note_hypothesis::Model),
        (status = 400, body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 409, description = "既にリンク済み", body = ErrorResponse),
        (status = 415, description = "Content-Type ヘッダが application/json ではない", body = ErrorResponse),
        (status = 422, description = "リクエストボディのパースに失敗", body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn create_note_hypothesis(
    State(state): State<AppState>,
    JsonPath(note_id): JsonPath<Uuid>,
    JsonBody(p): JsonBody<CreateNoteHypothesisRequest>,
) -> Result<(StatusCode, Json<note_hypothesis::Model>), AppError> {
    let note = find_note_or_404(&state.db, note_id).await?;

    let hypothesis = hypothesis::Entity::find_by_id(p.hypothesis_id)
        .one(&state.db)
        .await?;
    let hypothesis = match hypothesis {
        Some(h) if h.strategy_id == note.strategy_id => h,
        _ => {
            return Err(AppError::Validation(
                "hypothesis_id must belong to the same scope (strategy or global) as the note"
                    .into(),
            ));
        }
    };

    let model = note_hypothesis::ActiveModel {
        note_id: Set(note_id),
        hypothesis_id: Set(p.hypothesis_id),
        hypothesis_title: Set(hypothesis.title),
        hypothesis_body: Set(hypothesis.body),
        hypothesis_status: Set(hypothesis.status),
        created_at: sea_orm::ActiveValue::NotSet,
    };
    let created = note_hypothesis::Entity::insert(model)
        .exec_with_returning(&state.db)
        .await?;
    Ok((StatusCode::CREATED, Json(created)))
}

/// ノートと仮説の紐付けを解除する
#[utoipa::path(
    delete,
    path = "/api/notes/{id}/hypotheses/{hypothesis_id}",
    tag = "notes",
    params(
        ("id" = Uuid, Path, description = "ノート ID"),
        ("hypothesis_id" = Uuid, Path, description = "仮説 ID"),
    ),
    responses(
        (status = 204),
        (status = 400, body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn delete_note_hypothesis(
    State(state): State<AppState>,
    JsonPath((note_id, hypothesis_id)): JsonPath<(Uuid, Uuid)>,
) -> Result<StatusCode, AppError> {
    let res = note_hypothesis::Entity::delete_many()
        .filter(note_hypothesis::Column::NoteId.eq(note_id))
        .filter(note_hypothesis::Column::HypothesisId.eq(hypothesis_id))
        .exec(&state.db)
        .await?;
    if res.rows_affected == 0 {
        return Err(AppError::NotFound(format!(
            "note_hypothesis ({note_id}, {hypothesis_id}) not found"
        )));
    }
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;
    use sea_orm::ActiveModelTrait;
    use sea_orm::ActiveValue::{NotSet, Set};
    use sea_orm::{DatabaseConnection, EntityTrait, IntoActiveModel};
    use serde_json::json;
    use sqlx::PgPool;
    use uuid::Uuid;

    use crate::entities::{hypothesis, note};
    use crate::testing::{create_test_server_with_db, insert_test_strategy};

    async fn seed_note(db: &DatabaseConnection, strategy_id: Option<Uuid>) -> Uuid {
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
            graphs_json: Set(json!([])),
            execution_id: Set(None),
        }
        .insert(db)
        .await
        .expect("insert note");
        id
    }

    async fn seed_hypothesis(
        db: &DatabaseConnection,
        strategy_id: Option<Uuid>,
        title: &str,
        body: &str,
        status: &str,
    ) -> Uuid {
        let id = Uuid::new_v4();
        hypothesis::ActiveModel {
            hypothesis_id: Set(id),
            strategy_id: Set(strategy_id),
            title: Set(title.into()),
            body: Set(body.into()),
            status: Set(status.into()),
            related_note_ids: Set(vec![]),
            related_interest_ids: Set(vec![]),
            created_at: NotSet,
            updated_at: NotSet,
        }
        .insert(db)
        .await
        .expect("insert hypothesis");
        id
    }

    fn normalize(mut v: serde_json::Value) -> serde_json::Value {
        v["created_at"] = json!("<dyn>");
        v
    }

    #[sqlx::test(migrations = false)]
    async fn create_then_list_round_trips_with_snapshot(pool: PgPool) {
        let (db, server) = create_test_server_with_db(pool).await;
        let sid = insert_test_strategy(&db, "s").await;
        let nid = seed_note(&db, Some(sid)).await;
        let hid = seed_hypothesis(&db, Some(sid), "H1", "本文", "unverified").await;

        let created = server
            .post(&format!("/api/notes/{nid}/hypotheses"))
            .json(&json!({ "hypothesis_id": hid }))
            .await;
        created.assert_status(StatusCode::CREATED);
        let expected = json!({
            "note_id": nid,
            "hypothesis_id": hid,
            "hypothesis_title": "H1",
            "hypothesis_body": "本文",
            "hypothesis_status": "unverified",
            "created_at": "<dyn>",
        });
        assert_eq!(normalize(created.json()), expected);

        let list = server.get(&format!("/api/notes/{nid}/hypotheses")).await;
        list.assert_status_ok();
        let normalized: Vec<_> = list
            .json::<Vec<serde_json::Value>>()
            .into_iter()
            .map(normalize)
            .collect();
        assert_eq!(normalized, vec![expected]);
    }

    #[sqlx::test(migrations = false)]
    async fn snapshot_stays_pinned_after_hypothesis_is_edited(pool: PgPool) {
        let (db, server) = create_test_server_with_db(pool).await;
        let sid = insert_test_strategy(&db, "s").await;
        let nid = seed_note(&db, Some(sid)).await;
        let hid = seed_hypothesis(&db, Some(sid), "元のタイトル", "元の本文", "unverified").await;

        server
            .post(&format!("/api/notes/{nid}/hypotheses"))
            .json(&json!({ "hypothesis_id": hid }))
            .await
            .assert_status(StatusCode::CREATED);

        let current = hypothesis::Entity::find_by_id(hid)
            .one(&db)
            .await
            .expect("query hypothesis")
            .expect("hypothesis exists");
        let mut active = current.into_active_model();
        active.title = Set("編集後のタイトル".into());
        active.body = Set("編集後の本文".into());
        active.status = Set("supported".into());
        active.update(&db).await.expect("update hypothesis");

        let list = server.get(&format!("/api/notes/{nid}/hypotheses")).await;
        list.assert_status_ok();
        let normalized: Vec<_> = list
            .json::<Vec<serde_json::Value>>()
            .into_iter()
            .map(normalize)
            .collect();
        assert_eq!(
            normalized,
            vec![json!({
                "note_id": nid,
                "hypothesis_id": hid,
                "hypothesis_title": "元のタイトル",
                "hypothesis_body": "元の本文",
                "hypothesis_status": "unverified",
                "created_at": "<dyn>",
            })],
        );
    }

    #[sqlx::test(migrations = false)]
    async fn create_rejects_scope_mismatch(pool: PgPool) {
        let (db, server) = create_test_server_with_db(pool).await;
        let a = insert_test_strategy(&db, "a").await;
        let b = insert_test_strategy(&db, "b").await;

        for (label, note_strategy, hypothesis_strategy) in [
            (
                "strategy_note_vs_other_strategy_hypothesis",
                Some(a),
                Some(b),
            ),
            ("strategy_note_vs_global_hypothesis", Some(a), None),
            ("global_note_vs_strategy_hypothesis", None, Some(a)),
        ] {
            let nid = seed_note(&db, note_strategy).await;
            let hid = seed_hypothesis(&db, hypothesis_strategy, "t", "b", "unverified").await;

            let res = server
                .post(&format!("/api/notes/{nid}/hypotheses"))
                .json(&json!({ "hypothesis_id": hid }))
                .await;
            assert_eq!(
                res.status_code(),
                StatusCode::BAD_REQUEST,
                "case {label} did not return 400",
            );
        }
    }

    #[sqlx::test(migrations = false)]
    async fn create_for_unknown_note_returns_404(pool: PgPool) {
        let (db, server) = create_test_server_with_db(pool).await;
        let sid = insert_test_strategy(&db, "s").await;
        let hid = seed_hypothesis(&db, Some(sid), "t", "b", "unverified").await;

        let res = server
            .post(&format!("/api/notes/{}/hypotheses", Uuid::new_v4()))
            .json(&json!({ "hypothesis_id": hid }))
            .await;
        res.assert_status(StatusCode::NOT_FOUND);
    }

    #[sqlx::test(migrations = false)]
    async fn create_for_unknown_hypothesis_returns_400(pool: PgPool) {
        let (db, server) = create_test_server_with_db(pool).await;
        let sid = insert_test_strategy(&db, "s").await;
        let nid = seed_note(&db, Some(sid)).await;

        let res = server
            .post(&format!("/api/notes/{nid}/hypotheses"))
            .json(&json!({ "hypothesis_id": Uuid::new_v4() }))
            .await;
        res.assert_status(StatusCode::BAD_REQUEST);
    }

    #[sqlx::test(migrations = false)]
    async fn duplicate_create_returns_409(pool: PgPool) {
        let (db, server) = create_test_server_with_db(pool).await;
        let sid = insert_test_strategy(&db, "s").await;
        let nid = seed_note(&db, Some(sid)).await;
        let hid = seed_hypothesis(&db, Some(sid), "t", "b", "unverified").await;

        server
            .post(&format!("/api/notes/{nid}/hypotheses"))
            .json(&json!({ "hypothesis_id": hid }))
            .await
            .assert_status(StatusCode::CREATED);

        let dup = server
            .post(&format!("/api/notes/{nid}/hypotheses"))
            .json(&json!({ "hypothesis_id": hid }))
            .await;
        dup.assert_status(StatusCode::CONFLICT);
    }

    #[sqlx::test(migrations = false)]
    async fn delete_unlinks_and_unknown_pair_returns_404(pool: PgPool) {
        let (db, server) = create_test_server_with_db(pool).await;
        let sid = insert_test_strategy(&db, "s").await;
        let nid = seed_note(&db, Some(sid)).await;
        let hid = seed_hypothesis(&db, Some(sid), "t", "b", "unverified").await;

        server
            .post(&format!("/api/notes/{nid}/hypotheses"))
            .json(&json!({ "hypothesis_id": hid }))
            .await
            .assert_status(StatusCode::CREATED);

        let deleted = server
            .delete(&format!("/api/notes/{nid}/hypotheses/{hid}"))
            .await;
        deleted.assert_status(StatusCode::NO_CONTENT);

        let list = server.get(&format!("/api/notes/{nid}/hypotheses")).await;
        list.assert_status_ok();
        assert_eq!(
            list.json::<Vec<serde_json::Value>>(),
            Vec::<serde_json::Value>::new()
        );

        let again = server
            .delete(&format!("/api/notes/{nid}/hypotheses/{hid}"))
            .await;
        again.assert_status(StatusCode::NOT_FOUND);
    }
}
