#[cfg(test)]
mod tests {
    use super::super::assert_response_eq;
    use chrono::{DateTime, FixedOffset, NaiveDate};
    use sea_orm::ActiveModelTrait;
    use sea_orm::ActiveValue::Set;
    use uuid::Uuid;

    use crate::testing::{
        create_test_server_with_db, insert_test_note, insert_test_stock, insert_test_strategy,
    };
    use gateway_postgres::entities::prediction;

    async fn seed_prediction(
        db: &impl sea_orm::ConnectionTrait,
        strategy_id: Uuid,
        note_id: Option<Uuid>,
        target_stock_id: &str,
        benchmark_stock_id: &str,
        due_date: NaiveDate,
        created_at: DateTime<FixedOffset>,
    ) -> prediction::Model {
        prediction::ActiveModel {
            prediction_id: Set(Uuid::new_v4()),
            strategy_id: Set(strategy_id),
            note_id: Set(note_id),
            target_stock_id: Set(target_stock_id.into()),
            benchmark_stock_id: Set(benchmark_stock_id.into()),
            direction: Set("outperform".into()),
            probability: Set(rust_decimal::Decimal::new(65, 2)),
            base_date: Set(NaiveDate::from_ymd_opt(2099, 1, 2).expect("date")),
            due_date: Set(due_date),
            created_at: Set(created_at),
        }
        .insert(db)
        .await
        .expect("seed prediction")
    }

    fn ts(minute: u32) -> DateTime<FixedOffset> {
        DateTime::parse_from_rfc3339(&format!("2099-01-02T00:{minute:02}:00+00:00"))
            .expect("parse timestamp")
    }

    #[backend_test_macros::database_test]
    async fn list_returns_predictions_linked_to_note_in_creation_order(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let (db, server) = create_test_server_with_db(db).await;
        let sid = insert_test_strategy(&db, "s").await;
        let nid = insert_test_note(&db, sid, "t", "b").await;
        insert_test_stock(&db, "TEST_TARGET", "Test Target").await;
        insert_test_stock(&db, "TEST_BENCHMARK", "Test Benchmark").await;
        let other_note = insert_test_note(&db, sid, "other", "b").await;

        let first = seed_prediction(
            &db,
            sid,
            Some(nid),
            "TEST_TARGET",
            "TEST_BENCHMARK",
            NaiveDate::from_ymd_opt(2099, 2, 1).expect("date"),
            ts(0),
        )
        .await;
        let second = seed_prediction(
            &db,
            sid,
            Some(nid),
            "TEST_TARGET",
            "TEST_BENCHMARK",
            NaiveDate::from_ymd_opt(2099, 3, 1).expect("date"),
            ts(1),
        )
        .await;
        seed_prediction(
            &db,
            sid,
            Some(other_note),
            "TEST_TARGET",
            "TEST_BENCHMARK",
            NaiveDate::from_ymd_opt(2099, 3, 1).expect("date"),
            ts(2),
        )
        .await;

        let res = server.get(&format!("/api/notes/{nid}/predictions")).await;
        assert_response_eq(
            &res,
            axum::http::StatusCode::OK,
            Some(serde_json::json!([
                {
                    "prediction_id": first.prediction_id,
                    "strategy_id": sid,
                    "note_id": nid,
                    "target_stock_id": "TEST_TARGET",
                    "benchmark_stock_id": "TEST_BENCHMARK",
                    "direction": "outperform",
                    "probability": 0.65,
                    "base_date": "2099-01-02",
                    "due_date": "2099-02-01",
                    "created_at": first.created_at,
                },
                {
                    "prediction_id": second.prediction_id,
                    "strategy_id": sid,
                    "note_id": nid,
                    "target_stock_id": "TEST_TARGET",
                    "benchmark_stock_id": "TEST_BENCHMARK",
                    "direction": "outperform",
                    "probability": 0.65,
                    "base_date": "2099-01-02",
                    "due_date": "2099-03-01",
                    "created_at": second.created_at,
                },
            ])),
        );
    }

    #[backend_test_macros::database_test]
    async fn list_returns_empty_for_note_without_predictions(db: gateway_postgres::DatabaseHandle) {
        let (db, server) = create_test_server_with_db(db).await;
        let sid = insert_test_strategy(&db, "s").await;
        let nid = insert_test_note(&db, sid, "t", "b").await;

        let res = server.get(&format!("/api/notes/{nid}/predictions")).await;
        assert_response_eq(
            &res,
            axum::http::StatusCode::OK,
            Some(serde_json::json!([])),
        );
    }

    #[backend_test_macros::database_test]
    async fn list_for_unknown_note_returns_404(db: gateway_postgres::DatabaseHandle) {
        let (_db, server) = create_test_server_with_db(db).await;

        let missing_note_id = Uuid::new_v4();
        let res = server
            .get(&format!("/api/notes/{missing_note_id}/predictions"))
            .await;
        assert_response_eq(
            &res,
            axum::http::StatusCode::NOT_FOUND,
            Some(serde_json::json!({ "error": format!("note {missing_note_id} not found") })),
        );
    }
}
