use axum::Json;
use axum::extract::State;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
use uuid::Uuid;

use crate::AppState;
use crate::entities::prediction;
use crate::error::{AppError, ErrorResponse};
use crate::extractors::JsonPath;
use crate::handlers::notes::find_note_or_404;

/// ノートに紐づく予測一覧 (記録順)。
#[utoipa::path(
    get,
    path = "/api/notes/{id}/predictions",
    tag = "notes",
    params(("id" = Uuid, Path, description = "ノート ID")),
    responses(
        (status = 200, body = Vec<prediction::Model>),
        (status = 400, body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn list_note_predictions(
    State(state): State<AppState>,
    JsonPath(note_id): JsonPath<Uuid>,
) -> Result<Json<Vec<prediction::Model>>, AppError> {
    find_note_or_404(&state.db, note_id).await?;

    let rows = prediction::Entity::find()
        .filter(prediction::Column::NoteId.eq(note_id))
        .order_by_asc(prediction::Column::CreatedAt)
        .all(&state.db)
        .await?;
    Ok(Json(rows))
}

#[cfg(test)]
mod tests {
    use chrono::{DateTime, FixedOffset, NaiveDate};
    use sea_orm::ActiveModelTrait;
    use sea_orm::ActiveValue::Set;
    use sea_orm::DatabaseConnection;
    use sqlx::PgPool;
    use uuid::Uuid;

    use crate::entities::prediction;
    use crate::testing::{
        create_test_server_with_db, insert_test_note, insert_test_stock, insert_test_strategy,
    };

    async fn seed_prediction(
        db: &DatabaseConnection,
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
            base_date: Set(NaiveDate::from_ymd_opt(2026, 6, 1).expect("date")),
            due_date: Set(due_date),
            created_at: Set(created_at),
        }
        .insert(db)
        .await
        .expect("seed prediction")
    }

    fn ts(minute: u32) -> DateTime<FixedOffset> {
        DateTime::parse_from_rfc3339(&format!("2026-06-01T00:{minute:02}:00+00:00"))
            .expect("parse timestamp")
    }

    #[sqlx::test(migrations = false)]
    async fn list_returns_predictions_linked_to_note_in_creation_order(pool: PgPool) {
        let (db, server) = create_test_server_with_db(pool).await;
        let sid = insert_test_strategy(&db, "s").await;
        let nid = insert_test_note(&db, sid, "t", "b").await;
        insert_test_stock(&db, "TGT1", "Target").await;
        insert_test_stock(&db, "BM1", "Benchmark").await;
        let other_note = insert_test_note(&db, sid, "other", "b").await;

        let first = seed_prediction(
            &db,
            sid,
            Some(nid),
            "TGT1",
            "BM1",
            NaiveDate::from_ymd_opt(2026, 7, 1).expect("date"),
            ts(0),
        )
        .await;
        let second = seed_prediction(
            &db,
            sid,
            Some(nid),
            "TGT1",
            "BM1",
            NaiveDate::from_ymd_opt(2026, 8, 1).expect("date"),
            ts(1),
        )
        .await;
        seed_prediction(
            &db,
            sid,
            Some(other_note),
            "TGT1",
            "BM1",
            NaiveDate::from_ymd_opt(2026, 8, 1).expect("date"),
            ts(2),
        )
        .await;

        let res = server.get(&format!("/api/notes/{nid}/predictions")).await;
        res.assert_status_ok();
        assert_eq!(res.json::<Vec<prediction::Model>>(), vec![first, second]);
    }

    #[sqlx::test(migrations = false)]
    async fn list_returns_empty_for_note_without_predictions(pool: PgPool) {
        let (db, server) = create_test_server_with_db(pool).await;
        let sid = insert_test_strategy(&db, "s").await;
        let nid = insert_test_note(&db, sid, "t", "b").await;

        let res = server.get(&format!("/api/notes/{nid}/predictions")).await;
        res.assert_status_ok();
        assert_eq!(
            res.json::<Vec<prediction::Model>>(),
            Vec::<prediction::Model>::new()
        );
    }

    #[sqlx::test(migrations = false)]
    async fn list_for_unknown_note_returns_404(pool: PgPool) {
        let (_db, server) = create_test_server_with_db(pool).await;

        let res = server
            .get(&format!("/api/notes/{}/predictions", Uuid::new_v4()))
            .await;
        res.assert_status(axum::http::StatusCode::NOT_FOUND);
    }
}
