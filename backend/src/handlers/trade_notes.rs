use std::collections::HashMap;

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use sea_orm::ActiveValue::Set;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
use uuid::Uuid;

use crate::AppState;
use crate::entities::{note, trade, trade_note};
use crate::error::{AppError, ErrorResponse};
use crate::extractors::{JsonBody, JsonPath};
use crate::models::CreateTradeNoteRequest;

async fn find_trade_or_404(
    db: &sea_orm::DatabaseConnection,
    trade_id: Uuid,
) -> Result<trade::Model, AppError> {
    trade::Entity::find_by_id(trade_id)
        .one(db)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("trade {trade_id} not found")))
}

/// 取引に紐づく判断ノート一覧 (リンク作成順)
#[utoipa::path(
    get,
    path = "/api/trades/{id}/notes",
    tag = "trades",
    params(("id" = Uuid, Path, description = "取引 ID")),
    responses(
        (status = 200, body = Vec<note::Model>),
        (status = 400, body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn list_trade_notes(
    State(state): State<AppState>,
    JsonPath(trade_id): JsonPath<Uuid>,
) -> Result<Json<Vec<note::Model>>, AppError> {
    find_trade_or_404(&state.db, trade_id).await?;

    let links = trade_note::Entity::find()
        .filter(trade_note::Column::TradeId.eq(trade_id))
        .order_by_asc(trade_note::Column::CreatedAt)
        .all(&state.db)
        .await?;
    let note_ids: Vec<Uuid> = links.iter().map(|l| l.note_id).collect();

    let notes = note::Entity::find()
        .filter(note::Column::Id.is_in(note_ids.iter().copied()))
        .all(&state.db)
        .await?;
    let notes_by_id: HashMap<Uuid, note::Model> = notes.into_iter().map(|n| (n.id, n)).collect();

    // is_in() は順序を保証しないため、リンク作成順の note_ids を基準に組み立て直す
    let ordered = note_ids
        .into_iter()
        .filter_map(|id| notes_by_id.get(&id).cloned())
        .collect();
    Ok(Json(ordered))
}

/// 取引に判断ノートを紐付ける
#[utoipa::path(
    post,
    path = "/api/trades/{id}/notes",
    tag = "trades",
    params(("id" = Uuid, Path, description = "取引 ID")),
    request_body = CreateTradeNoteRequest,
    responses(
        (status = 201, body = trade_note::Model),
        (status = 400, body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 409, description = "既にリンク済み", body = ErrorResponse),
        (status = 415, description = "Content-Type ヘッダが application/json ではない", body = ErrorResponse),
        (status = 422, description = "リクエストボディのパースに失敗", body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn create_trade_note(
    State(state): State<AppState>,
    JsonPath(trade_id): JsonPath<Uuid>,
    JsonBody(p): JsonBody<CreateTradeNoteRequest>,
) -> Result<(StatusCode, Json<trade_note::Model>), AppError> {
    let trade = find_trade_or_404(&state.db, trade_id).await?;

    let note = note::Entity::find_by_id(p.note_id).one(&state.db).await?;
    match note {
        Some(n) if n.strategy_id == Some(trade.strategy_id) => {}
        _ => {
            return Err(AppError::Validation(
                "note_id must belong to the same strategy as the trade".into(),
            ));
        }
    }

    let model = trade_note::ActiveModel {
        trade_id: Set(trade_id),
        note_id: Set(p.note_id),
        created_at: sea_orm::ActiveValue::NotSet,
    };
    let created = trade_note::Entity::insert(model)
        .exec_with_returning(&state.db)
        .await?;
    Ok((StatusCode::CREATED, Json(created)))
}

/// 取引と判断ノートの紐付けを解除する
#[utoipa::path(
    delete,
    path = "/api/trades/{id}/notes/{note_id}",
    tag = "trades",
    params(
        ("id" = Uuid, Path, description = "取引 ID"),
        ("note_id" = Uuid, Path, description = "ノート ID"),
    ),
    responses(
        (status = 204),
        (status = 400, body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn delete_trade_note(
    State(state): State<AppState>,
    JsonPath((trade_id, note_id)): JsonPath<(Uuid, Uuid)>,
) -> Result<StatusCode, AppError> {
    let res = trade_note::Entity::delete_many()
        .filter(trade_note::Column::TradeId.eq(trade_id))
        .filter(trade_note::Column::NoteId.eq(note_id))
        .exec(&state.db)
        .await?;
    if res.rows_affected == 0 {
        return Err(AppError::NotFound(format!(
            "trade_note ({trade_id}, {note_id}) not found"
        )));
    }
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;
    use rust_decimal::Decimal;
    use sea_orm::ActiveModelTrait;
    use sea_orm::ActiveValue::{NotSet, Set};
    use sea_orm::DatabaseConnection;
    use serde_json::json;
    use sqlx::PgPool;
    use uuid::Uuid;

    use crate::entities::{note, trade};
    use crate::testing::{create_test_server_with_db, insert_test_strategy};

    async fn seed_trade(db: &DatabaseConnection, strategy_id: Uuid) -> Uuid {
        let id = Uuid::new_v4();
        trade::ActiveModel {
            id: Set(id),
            strategy_id: Set(strategy_id),
            symbol: Set("7203".into()),
            side: Set("buy".into()),
            qty: Set(Decimal::from(100)),
            price: Set(Decimal::from(1000)),
            fee: Set(Decimal::ZERO),
            date: Set(chrono::NaiveDate::from_ymd_opt(2026, 1, 1).expect("date")),
            source: Set("manual".into()),
            note: Set(None),
            created_at: NotSet,
            updated_at: NotSet,
        }
        .insert(db)
        .await
        .expect("insert trade");
        id
    }

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

    fn normalize_trade_note(mut v: serde_json::Value) -> serde_json::Value {
        v["created_at"] = json!("<dyn>");
        v
    }

    fn normalize_note(mut v: serde_json::Value) -> serde_json::Value {
        v["created_at"] = json!("<dyn>");
        v["updated_at"] = json!("<dyn>");
        v
    }

    #[sqlx::test(migrations = false)]
    async fn create_then_list_round_trips(pool: PgPool) {
        let (db, server) = create_test_server_with_db(pool).await;
        let sid = insert_test_strategy(&db, "s").await;
        let tid = seed_trade(&db, sid).await;
        let nid = seed_note(&db, Some(sid)).await;

        let created = server
            .post(&format!("/api/trades/{tid}/notes"))
            .json(&json!({ "note_id": nid }))
            .await;
        created.assert_status(StatusCode::CREATED);
        assert_eq!(
            normalize_trade_note(created.json()),
            json!({
                "trade_id": tid,
                "note_id": nid,
                "created_at": "<dyn>",
            }),
        );

        let list = server.get(&format!("/api/trades/{tid}/notes")).await;
        list.assert_status_ok();
        let normalized: Vec<_> = list
            .json::<Vec<serde_json::Value>>()
            .into_iter()
            .map(normalize_note)
            .collect();
        assert_eq!(
            normalized,
            vec![json!({
                "id": nid,
                "strategy_id": sid,
                "title": "t",
                "body_md": "b",
                "frontmatter_json": {},
                "type_tag": null,
                "status": "unread",
                "trigger": null,
                "trigger_label": null,
                "created_by_kind": "human",
                "created_at": "<dyn>",
                "updated_at": "<dyn>",
                "graphs_json": [],
                "execution_id": null,
            })],
        );
    }

    #[sqlx::test(migrations = false)]
    async fn list_preserves_link_creation_order(pool: PgPool) {
        let (db, server) = create_test_server_with_db(pool).await;
        let sid = insert_test_strategy(&db, "s").await;
        let tid = seed_trade(&db, sid).await;
        let n1 = seed_note(&db, Some(sid)).await;
        let n2 = seed_note(&db, Some(sid)).await;

        server
            .post(&format!("/api/trades/{tid}/notes"))
            .json(&json!({ "note_id": n2 }))
            .await
            .assert_status(StatusCode::CREATED);
        server
            .post(&format!("/api/trades/{tid}/notes"))
            .json(&json!({ "note_id": n1 }))
            .await
            .assert_status(StatusCode::CREATED);

        let list = server.get(&format!("/api/trades/{tid}/notes")).await;
        list.assert_status_ok();
        let ids: Vec<Uuid> = list
            .json::<Vec<serde_json::Value>>()
            .iter()
            .map(|v| Uuid::parse_str(v["id"].as_str().unwrap()).unwrap())
            .collect();
        assert_eq!(ids, vec![n2, n1]);
    }

    #[sqlx::test(migrations = false)]
    async fn create_rejects_cross_strategy_note(pool: PgPool) {
        let (db, server) = create_test_server_with_db(pool).await;
        let a = insert_test_strategy(&db, "a").await;
        let b = insert_test_strategy(&db, "b").await;
        let tid = seed_trade(&db, a).await;
        let nid = seed_note(&db, Some(b)).await;

        let res = server
            .post(&format!("/api/trades/{tid}/notes"))
            .json(&json!({ "note_id": nid }))
            .await;
        res.assert_status(StatusCode::BAD_REQUEST);
    }

    #[sqlx::test(migrations = false)]
    async fn create_for_unknown_trade_returns_404(pool: PgPool) {
        let (db, server) = create_test_server_with_db(pool).await;
        let sid = insert_test_strategy(&db, "s").await;
        let nid = seed_note(&db, Some(sid)).await;

        let res = server
            .post(&format!("/api/trades/{}/notes", Uuid::new_v4()))
            .json(&json!({ "note_id": nid }))
            .await;
        res.assert_status(StatusCode::NOT_FOUND);
    }

    #[sqlx::test(migrations = false)]
    async fn create_for_unknown_note_returns_400(pool: PgPool) {
        let (db, server) = create_test_server_with_db(pool).await;
        let sid = insert_test_strategy(&db, "s").await;
        let tid = seed_trade(&db, sid).await;

        let res = server
            .post(&format!("/api/trades/{tid}/notes"))
            .json(&json!({ "note_id": Uuid::new_v4() }))
            .await;
        res.assert_status(StatusCode::BAD_REQUEST);
    }

    #[sqlx::test(migrations = false)]
    async fn duplicate_create_returns_409(pool: PgPool) {
        let (db, server) = create_test_server_with_db(pool).await;
        let sid = insert_test_strategy(&db, "s").await;
        let tid = seed_trade(&db, sid).await;
        let nid = seed_note(&db, Some(sid)).await;

        server
            .post(&format!("/api/trades/{tid}/notes"))
            .json(&json!({ "note_id": nid }))
            .await
            .assert_status(StatusCode::CREATED);

        let dup = server
            .post(&format!("/api/trades/{tid}/notes"))
            .json(&json!({ "note_id": nid }))
            .await;
        dup.assert_status(StatusCode::CONFLICT);
    }

    #[sqlx::test(migrations = false)]
    async fn delete_unlinks_and_unknown_pair_returns_404(pool: PgPool) {
        let (db, server) = create_test_server_with_db(pool).await;
        let sid = insert_test_strategy(&db, "s").await;
        let tid = seed_trade(&db, sid).await;
        let nid = seed_note(&db, Some(sid)).await;

        server
            .post(&format!("/api/trades/{tid}/notes"))
            .json(&json!({ "note_id": nid }))
            .await
            .assert_status(StatusCode::CREATED);

        let deleted = server
            .delete(&format!("/api/trades/{tid}/notes/{nid}"))
            .await;
        deleted.assert_status(StatusCode::NO_CONTENT);

        let list = server.get(&format!("/api/trades/{tid}/notes")).await;
        list.assert_status_ok();
        assert_eq!(
            list.json::<Vec<serde_json::Value>>(),
            Vec::<serde_json::Value>::new()
        );

        let again = server
            .delete(&format!("/api/trades/{tid}/notes/{nid}"))
            .await;
        again.assert_status(StatusCode::NOT_FOUND);
    }
}
