#[cfg(test)]
mod tests {
    use super::super::assert_response_eq;
    use axum::http::StatusCode;
    use rust_decimal::Decimal;
    use sea_orm::ActiveModelTrait;
    use sea_orm::ActiveValue::{NotSet, Set};

    use serde_json::json;
    use uuid::Uuid;

    use crate::testing::{create_test_server_with_db, insert_test_note, insert_test_strategy};
    use gateway_postgres::entities::{trade, trade_note};

    async fn seed_trade(db: &impl sea_orm::ConnectionTrait, strategy_id: Uuid) -> Uuid {
        let id = Uuid::new_v4();
        trade::ActiveModel {
            id: Set(id),
            strategy_id: Set(strategy_id),
            symbol: Set("demo-code".into()),
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

    async fn seed_note(db: &gateway_postgres::DatabaseHandle) -> Uuid {
        insert_test_note(db, "t", "b").await
    }

    fn normalize_trade_note(mut v: serde_json::Value) -> serde_json::Value {
        for key in ["created_at", "note_version_id"] {
            if let Some(value) = v.get_mut(key) {
                *value = json!("<dyn>");
            }
        }
        v
    }

    fn normalize_note(mut v: serde_json::Value) -> serde_json::Value {
        for key in ["created_at", "updated_at", "version_id"] {
            if let Some(value) = v.get_mut(key) {
                *value = json!("<dyn>");
            }
        }
        v
    }

    fn expected_note(id: Uuid) -> serde_json::Value {
        json!({
            "id": id,
            "version_id": "<dyn>",
            "version_no": 1,
            "is_current": true,
            "title": "t",
            "body_md": "b",
            "frontmatter_json": {},
            "tags": [],
            "kind": null,
            "status": "approved",
            "trigger": null,
            "trigger_label": null,
            "created_by_kind": "human",
            "created_at": "<dyn>",
            "updated_at": "<dyn>",
            "graphs_json": [],
            "execution_id": null,
        })
    }

    async fn assert_trade_note_created(
        server: &axum_test::TestServer,
        trade_id: Uuid,
        note_id: Uuid,
    ) {
        let response = server
            .post(&format!("/api/trades/{trade_id}/notes"))
            .json(&json!({ "note_id": note_id }))
            .await;
        assert_eq!(
            (
                response.status_code(),
                normalize_trade_note(response.json()),
            ),
            (
                StatusCode::CREATED,
                json!({
                    "trade_id": trade_id,
                    "note_id": note_id,
                    "note_version_id": "<dyn>",
                    "created_at": "<dyn>",
                }),
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn create_then_list_round_trips(db: gateway_postgres::DatabaseHandle) {
        let (db, server) = create_test_server_with_db(db).await;
        let sid = insert_test_strategy(&db, "s").await;
        let tid = seed_trade(&db, sid).await;
        let nid = seed_note(&db).await;

        let created = server
            .post(&format!("/api/trades/{tid}/notes"))
            .json(&json!({ "note_id": nid }))
            .await;
        assert_eq!(
            (created.status_code(), normalize_trade_note(created.json()),),
            (
                StatusCode::CREATED,
                json!({
                    "trade_id": tid,
                    "note_id": nid,
                    "note_version_id": "<dyn>",
                    "created_at": "<dyn>",
                }),
            ),
        );

        let list = server.get(&format!("/api/trades/{tid}/notes")).await;
        let normalized: Vec<_> = list
            .json::<Vec<serde_json::Value>>()
            .into_iter()
            .map(normalize_note)
            .collect();
        assert_eq!(
            (list.status_code(), normalized),
            (StatusCode::OK, vec![expected_note(nid)]),
        );
    }

    #[backend_test_macros::database_test]
    async fn list_preserves_link_creation_order(db: gateway_postgres::DatabaseHandle) {
        let (db, server) = create_test_server_with_db(db).await;
        let sid = insert_test_strategy(&db, "s").await;
        let tid = seed_trade(&db, sid).await;
        let n1 = seed_note(&db).await;
        let n2 = seed_note(&db).await;

        assert_trade_note_created(&server, tid, n2).await;
        assert_trade_note_created(&server, tid, n1).await;

        let link_time = chrono::DateTime::<chrono::Utc>::UNIX_EPOCH.fixed_offset();
        trade_note::ActiveModel {
            trade_id: Set(tid),
            note_id: Set(n2),
            created_at: Set(link_time),
            ..Default::default()
        }
        .update(&db)
        .await
        .expect("set first trade-note link time");
        trade_note::ActiveModel {
            trade_id: Set(tid),
            note_id: Set(n1),
            created_at: Set(link_time + chrono::Duration::seconds(1)),
            ..Default::default()
        }
        .update(&db)
        .await
        .expect("set second trade-note link time");

        let list = server.get(&format!("/api/trades/{tid}/notes")).await;
        let normalized: Vec<_> = list
            .json::<Vec<serde_json::Value>>()
            .iter()
            .cloned()
            .map(normalize_note)
            .collect();
        assert_eq!(
            (list.status_code(), normalized),
            (StatusCode::OK, vec![expected_note(n2), expected_note(n1)],),
        );
    }

    #[backend_test_macros::database_test]
    async fn create_links_multiple_notes_to_trade(db: gateway_postgres::DatabaseHandle) {
        let (db, server) = create_test_server_with_db(db).await;
        let a = insert_test_strategy(&db, "a").await;
        let tid = seed_trade(&db, a).await;
        let first_note_id = seed_note(&db).await;
        let second_note_id = seed_note(&db).await;

        assert_trade_note_created(&server, tid, first_note_id).await;
        assert_trade_note_created(&server, tid, second_note_id).await;
    }

    #[backend_test_macros::database_test]
    async fn create_for_unknown_trade_returns_404(db: gateway_postgres::DatabaseHandle) {
        let (db, server) = create_test_server_with_db(db).await;
        let nid = seed_note(&db).await;

        let missing_trade_id = Uuid::new_v4();
        let res = server
            .post(&format!("/api/trades/{missing_trade_id}/notes"))
            .json(&json!({ "note_id": nid }))
            .await;
        assert_response_eq(
            &res,
            StatusCode::NOT_FOUND,
            Some(json!({ "error": format!("trade {missing_trade_id} not found") })),
        );
    }

    #[backend_test_macros::database_test]
    async fn create_for_unknown_note_returns_404(db: gateway_postgres::DatabaseHandle) {
        let (db, server) = create_test_server_with_db(db).await;
        let sid = insert_test_strategy(&db, "s").await;
        let tid = seed_trade(&db, sid).await;
        let missing_note_id = Uuid::new_v4();

        let res = server
            .post(&format!("/api/trades/{tid}/notes"))
            .json(&json!({ "note_id": missing_note_id }))
            .await;
        assert_response_eq(
            &res,
            StatusCode::NOT_FOUND,
            Some(json!({ "error": format!("note {missing_note_id} not found") })),
        );
    }

    #[backend_test_macros::database_test]
    async fn duplicate_create_returns_409(db: gateway_postgres::DatabaseHandle) {
        let (db, server) = create_test_server_with_db(db).await;
        let sid = insert_test_strategy(&db, "s").await;
        let tid = seed_trade(&db, sid).await;
        let nid = seed_note(&db).await;

        assert_trade_note_created(&server, tid, nid).await;

        let dup = server
            .post(&format!("/api/trades/{tid}/notes"))
            .json(&json!({ "note_id": nid }))
            .await;
        assert_response_eq(
            &dup,
            StatusCode::CONFLICT,
            Some(json!({ "error": "resource already exists" })),
        );
    }

    #[backend_test_macros::database_test]
    async fn delete_unlinks_and_unknown_pair_returns_404(db: gateway_postgres::DatabaseHandle) {
        let (db, server) = create_test_server_with_db(db).await;
        let sid = insert_test_strategy(&db, "s").await;
        let tid = seed_trade(&db, sid).await;
        let nid = seed_note(&db).await;

        assert_trade_note_created(&server, tid, nid).await;

        let deleted = server
            .delete(&format!("/api/trades/{tid}/notes/{nid}"))
            .await;
        assert_response_eq(&deleted, StatusCode::NO_CONTENT, None);

        let list = server.get(&format!("/api/trades/{tid}/notes")).await;
        assert_response_eq(&list, StatusCode::OK, Some(json!([])));

        let again = server
            .delete(&format!("/api/trades/{tid}/notes/{nid}"))
            .await;
        assert_response_eq(
            &again,
            StatusCode::NOT_FOUND,
            Some(json!({
                "error": format!("trade_note ({tid}, {nid}) not found")
            })),
        );
    }
}
