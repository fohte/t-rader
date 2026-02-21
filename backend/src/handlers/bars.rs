use axum::Json;
use axum::extract::State;
use chrono::NaiveDate;
use serde::Deserialize;
use utoipa::IntoParams;

use crate::AppState;
use crate::entities::bars;
use crate::error::{AppError, ErrorResponse};
use crate::extractors::JsonQuery;
use crate::repositories;

/// バーデータ取得のクエリパラメータ
#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct BarsQueryParams {
    /// 銘柄コード (必須)
    pub instrument_id: String,
    /// 時間足 (デフォルト: "1d")
    #[serde(default = "default_timeframe")]
    pub timeframe: String,
    /// 取得開始日 (YYYY-MM-DD, inclusive)
    pub from: Option<NaiveDate>,
    /// 取得終了日 (YYYY-MM-DD, inclusive)
    pub to: Option<NaiveDate>,
}

fn default_timeframe() -> String {
    "1d".to_string()
}

/// バーデータを取得する
#[utoipa::path(
    get,
    path = "/api/bars",
    tag = "bars",
    params(BarsQueryParams),
    responses(
        (status = 200, description = "バーデータ一覧", body = Vec<bars::Model>),
        (status = 400, description = "バリデーションエラー", body = ErrorResponse),
        (status = 500, description = "内部サーバーエラー", body = ErrorResponse),
    )
)]
pub async fn list_bars(
    State(state): State<AppState>,
    JsonQuery(params): JsonQuery<BarsQueryParams>,
) -> Result<Json<Vec<bars::Model>>, AppError> {
    if params.instrument_id.trim().is_empty() {
        return Err(AppError::Validation(
            "instrument_id must not be empty".to_string(),
        ));
    }

    let valid_timeframes = ["1d"];
    if !valid_timeframes.contains(&params.timeframe.as_str()) {
        return Err(AppError::Validation(format!(
            "invalid timeframe: {}. valid values: {:?}",
            params.timeframe, valid_timeframes
        )));
    }

    // NaiveDate -> DateTime<FixedOffset> に変換
    // from: その日の 00:00:00 UTC
    // to: その日の 23:59:59 UTC (inclusive)
    let from = params
        .from
        .and_then(|d| d.and_hms_opt(0, 0, 0))
        .map(|dt| dt.and_utc().fixed_offset());

    let to = params
        .to
        .and_then(|d| d.and_hms_opt(23, 59, 59))
        .map(|dt| dt.and_utc().fixed_offset());

    let query = repositories::bars::BarsQuery {
        instrument_id: params.instrument_id,
        timeframe: params.timeframe,
        from,
        to,
    };

    let bars = repositories::bars::find_bars(&state.db, query).await?;

    Ok(Json(bars))
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;
    use chrono::{NaiveDate, TimeZone, Utc};
    use rust_decimal::Decimal;
    use sea_orm::sea_query::OnConflict;
    use sea_orm::{DatabaseConnection, EntityTrait, Set, SqlxPostgresConnector};
    use sqlx::PgPool;

    use crate::entities::instruments;
    use crate::models::bar::{Bar, Timeframe};
    use crate::repositories;
    use crate::testing::create_test_server;

    /// テスト用の instrument を DB に挿入する
    async fn insert_test_instrument(db: &DatabaseConnection, id: &str) {
        instruments::Entity::insert(instruments::ActiveModel {
            id: Set(id.to_string()),
            name: Set(format!("Test {id}")),
            market: Set("TSE".to_string()),
            sector: Set(None),
        })
        .on_conflict(
            OnConflict::column(instruments::Column::Id)
                .do_nothing()
                .to_owned(),
        )
        .exec_without_returning(db)
        .await
        .expect("failed to insert test instrument");
    }

    /// テスト用のバーデータを生成する
    fn make_test_bar(instrument_id: &str, date: NaiveDate, close: i64) -> Bar {
        let timestamp = date
            .and_hms_opt(0, 0, 0)
            .map(|dt| Utc.from_utc_datetime(&dt))
            .expect("invalid date");
        Bar {
            instrument_id: instrument_id.to_string(),
            timeframe: Timeframe::Daily,
            timestamp,
            open: Decimal::new(close, 0),
            high: Decimal::new(close + 10, 0),
            low: Decimal::new(close - 10, 0),
            close: Decimal::new(close, 0),
            volume: 1000,
        }
    }

    /// テストサーバーとセットアップ済みの DB 接続を返す
    ///
    /// PgPool を clone してサーバー用と直接操作用に分ける。
    async fn setup(pool: PgPool) -> (axum_test::TestServer, DatabaseConnection) {
        let db = SqlxPostgresConnector::from_sqlx_postgres_pool(pool.clone());
        let server = create_test_server(pool).await;
        (server, db)
    }

    #[sqlx::test(migrations = false)]
    async fn list_bars_returns_200_with_data(pool: PgPool) {
        let (server, db) = setup(pool).await;
        insert_test_instrument(&db, "7203").await;

        let bars = vec![make_test_bar(
            "7203",
            NaiveDate::from_ymd_opt(2025, 1, 6).expect("invalid date"),
            100,
        )];
        repositories::bars::upsert_bars(&db, bars)
            .await
            .expect("upsert failed");

        let response = server.get("/api/bars?instrument_id=7203").await;
        response.assert_status_ok();

        let body: Vec<serde_json::Value> = response.json();
        assert_eq!(body.len(), 1);
        assert_eq!(body[0]["instrument_id"], "7203");
        assert_eq!(body[0]["timeframe"], "1d");
    }

    #[sqlx::test(migrations = false)]
    async fn list_bars_with_invalid_params_returns_400(pool: PgPool) {
        let server = create_test_server(pool).await;

        let cases = [
            ("empty_instrument_id", "?instrument_id="),
            ("invalid_timeframe", "?instrument_id=7203&timeframe=5m"),
        ];

        for (name, query) in cases {
            let response = server.get(&format!("/api/bars{query}")).await;
            response.assert_status(StatusCode::BAD_REQUEST);
            assert!(
                response.text().contains("error"),
                "case '{name}' should return JSON error body"
            );
        }
    }

    #[sqlx::test(migrations = false)]
    async fn list_bars_returns_empty_when_no_data(pool: PgPool) {
        let server = create_test_server(pool).await;

        let response = server.get("/api/bars?instrument_id=9999").await;
        response.assert_status_ok();

        let body: Vec<serde_json::Value> = response.json();
        assert!(body.is_empty());
    }

    #[sqlx::test(migrations = false)]
    async fn list_bars_with_date_range_filters_correctly(pool: PgPool) {
        let (server, db) = setup(pool).await;
        insert_test_instrument(&db, "7203").await;

        let bars = vec![
            make_test_bar(
                "7203",
                NaiveDate::from_ymd_opt(2025, 1, 6).expect("invalid date"),
                100,
            ),
            make_test_bar(
                "7203",
                NaiveDate::from_ymd_opt(2025, 1, 7).expect("invalid date"),
                105,
            ),
            make_test_bar(
                "7203",
                NaiveDate::from_ymd_opt(2025, 1, 8).expect("invalid date"),
                103,
            ),
        ];
        repositories::bars::upsert_bars(&db, bars)
            .await
            .expect("upsert failed");

        let response = server
            .get("/api/bars?instrument_id=7203&from=2025-01-07&to=2025-01-07")
            .await;
        response.assert_status_ok();

        let body: Vec<serde_json::Value> = response.json();
        assert_eq!(body.len(), 1);
        assert_eq!(body[0]["instrument_id"], "7203");
        assert_eq!(body[0]["close"], 105.0);
    }
}
