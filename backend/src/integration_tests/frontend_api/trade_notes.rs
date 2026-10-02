#[cfg(test)]
mod tests {
    use axum::http::StatusCode;
    use rust_decimal::Decimal;
    use sea_orm::ActiveModelTrait;
    use sea_orm::ActiveValue::{NotSet, Set};

    use serde_json::json;
    use uuid::Uuid;

    use crate::testing::{
        create_test_server_with_db, insert_test_note_in_scope, insert_test_strategy,
    };
    use gateway_postgres::entities::{trade, trade_note};

    async fn seed_trade(db: &impl sea_orm::ConnectionTrait, strategy_id: Uuid) -> Uuid {
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

    async fn seed_note(db: &gateway_postgres::DatabaseHandle, strategy_id: Option<Uuid>) -> Uuid {
        insert_test_note_in_scope(db, strategy_id, "t", "b").await
    }

    fn normalize_trade_note(mut v: serde_json::Value) -> serde_json::Value {
        v["created_at"] = json!("<dyn>");
        v["note_version_id"] = json!("<dyn>");
        v
    }

    fn normalize_note(mut v: serde_json::Value) -> serde_json::Value {
        v["created_at"] = json!("<dyn>");
        v["updated_at"] = json!("<dyn>");
        v["version_id"] = json!("<dyn>");
        v
    }

    #[backend_test_macros::database_test]
    async fn create_then_list_round_trips(db: gateway_postgres::DatabaseHandle) {
        let (db, server) = create_test_server_with_db(db).await;
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
                "note_version_id": "<dyn>",
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
                "version_id": "<dyn>",
                "version_no": 1,
                "is_current": true,
                "strategy_id": sid,
                "title": "t",
                "body_md": "b",
                "frontmatter_json": {},
                "kind": null,
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

    #[backend_test_macros::database_test]
    async fn list_preserves_link_creation_order(db: gateway_postgres::DatabaseHandle) {
        let (db, server) = create_test_server_with_db(db).await;
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
        list.assert_status_ok();
        let ids: Vec<Uuid> = list
            .json::<Vec<serde_json::Value>>()
            .iter()
            .map(|v| Uuid::parse_str(v["id"].as_str().unwrap()).unwrap())
            .collect();
        assert_eq!(ids, vec![n2, n1]);
    }

    #[backend_test_macros::database_test]
    async fn create_rejects_cross_strategy_note(db: gateway_postgres::DatabaseHandle) {
        let (db, server) = create_test_server_with_db(db).await;
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

    #[backend_test_macros::database_test]
    async fn create_for_unknown_trade_returns_404(db: gateway_postgres::DatabaseHandle) {
        let (db, server) = create_test_server_with_db(db).await;
        let sid = insert_test_strategy(&db, "s").await;
        let nid = seed_note(&db, Some(sid)).await;

        let res = server
            .post(&format!("/api/trades/{}/notes", Uuid::new_v4()))
            .json(&json!({ "note_id": nid }))
            .await;
        res.assert_status(StatusCode::NOT_FOUND);
    }

    #[backend_test_macros::database_test]
    async fn create_for_unknown_note_returns_400(db: gateway_postgres::DatabaseHandle) {
        let (db, server) = create_test_server_with_db(db).await;
        let sid = insert_test_strategy(&db, "s").await;
        let tid = seed_trade(&db, sid).await;

        let res = server
            .post(&format!("/api/trades/{tid}/notes"))
            .json(&json!({ "note_id": Uuid::new_v4() }))
            .await;
        res.assert_status(StatusCode::BAD_REQUEST);
    }

    #[backend_test_macros::database_test]
    async fn duplicate_create_returns_409(db: gateway_postgres::DatabaseHandle) {
        let (db, server) = create_test_server_with_db(db).await;
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

    #[backend_test_macros::database_test]
    async fn delete_unlinks_and_unknown_pair_returns_404(db: gateway_postgres::DatabaseHandle) {
        let (db, server) = create_test_server_with_db(db).await;
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
