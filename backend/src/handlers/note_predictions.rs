use axum::Json;
use axum::extract::State;
use core_application::prediction::{PredictionRepositoryError, PredictionUseCaseError};
use core_application::unit_of_work::UnitOfWorkError;
use uuid::Uuid;

use crate::AppState;
use crate::error::{AppError, ErrorResponse};
use crate::extractors::JsonPath;
use crate::models::PredictionResponse;

/// ノートに紐づく予測一覧 (記録順)。
#[utoipa::path(
    get,
    path = "/api/notes/{id}/predictions",
    tag = "notes",
    params(("id" = Uuid, Path, description = "ノート ID")),
    responses(
        (status = 200, body = Vec<PredictionResponse>),
        (status = 400, body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn list_note_predictions(
    State(state): State<AppState>,
    JsonPath(note_id): JsonPath<Uuid>,
) -> Result<Json<Vec<PredictionResponse>>, AppError> {
    let rows = state
        .use_cases
        .predictions()
        .list_by_note(note_id)
        .await
        .map_err(map_prediction_error)?;
    Ok(Json(rows.into_iter().map(Into::into).collect()))
}

fn map_prediction_error(error: PredictionUseCaseError) -> AppError {
    match error {
        PredictionUseCaseError::Validation(message) => AppError::Validation(message),
        PredictionUseCaseError::NoteNotFound(note_id) => {
            AppError::NotFound(format!("note {note_id} not found"))
        }
        PredictionUseCaseError::Forbidden(note_id) => {
            AppError::Validation(format!("note {note_id} belongs to another strategy"))
        }
        PredictionUseCaseError::Repository(PredictionRepositoryError::Database(error)) => {
            error.into()
        }
        PredictionUseCaseError::UnitOfWork(UnitOfWorkError::Begin(error))
        | PredictionUseCaseError::UnitOfWork(UnitOfWorkError::Commit(error)) => error.into(),
        PredictionUseCaseError::Repository(PredictionRepositoryError::InvalidTransaction)
        | PredictionUseCaseError::UnitOfWork(UnitOfWorkError::InvalidTransaction) => {
            AppError::Internal("prediction transaction has an unexpected type".into())
        }
    }
}

#[cfg(test)]
mod tests {
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
        res.assert_status_ok();
        assert_eq!(
            res.json::<serde_json::Value>(),
            serde_json::json!([
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
            ]),
        );
    }

    #[backend_test_macros::database_test]
    async fn list_returns_empty_for_note_without_predictions(db: gateway_postgres::DatabaseHandle) {
        let (db, server) = create_test_server_with_db(db).await;
        let sid = insert_test_strategy(&db, "s").await;
        let nid = insert_test_note(&db, sid, "t", "b").await;

        let res = server.get(&format!("/api/notes/{nid}/predictions")).await;
        res.assert_status_ok();
        assert_eq!(res.json::<serde_json::Value>(), serde_json::json!([]));
    }

    #[backend_test_macros::database_test]
    async fn list_for_unknown_note_returns_404(db: gateway_postgres::DatabaseHandle) {
        let (_db, server) = create_test_server_with_db(db).await;

        let res = server
            .get(&format!("/api/notes/{}/predictions", Uuid::new_v4()))
            .await;
        res.assert_status(axum::http::StatusCode::NOT_FOUND);
    }
}
