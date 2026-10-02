#[cfg(test)]
mod tests {
    use super::super::assert_response_eq;
    use crate::models::bar::{Bar, Timeframe};
    use crate::testing::{create_test_server, create_test_server_with_db};
    use axum::http::StatusCode;
    use chrono::{NaiveDate, TimeZone, Utc};
    use gateway_postgres::entities::instruments;
    use gateway_postgres::repositories;
    use rust_decimal::Decimal;
    use sea_orm::sea_query::OnConflict;
    use sea_orm::{EntityTrait, Set};

    /// テスト用の instrument を DB に挿入する
    async fn insert_test_instrument(db: &impl sea_orm::ConnectionTrait, id: &str) {
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

    #[backend_test_macros::database_test]
    async fn list_bars_returns_200_with_data(db: gateway_postgres::DatabaseHandle) {
        let (db, server) = create_test_server_with_db(db).await;
        insert_test_instrument(&db, "TEST-INSTRUMENT").await;

        let bars = vec![make_test_bar(
            "TEST-INSTRUMENT",
            NaiveDate::from_ymd_opt(2025, 1, 6).expect("invalid date"),
            100,
        )];
        repositories::bars::upsert_bars(&db, bars)
            .await
            .expect("upsert failed");

        let response = server.get("/api/bars?instrument_id=TEST-INSTRUMENT").await;
        assert_response_eq(
            &response,
            StatusCode::OK,
            Some(serde_json::json!([{
                "instrument_id": "TEST-INSTRUMENT",
                "timeframe": "1d",
                "timestamp": "2025-01-06T00:00:00Z",
                "open": 100,
                "high": 110,
                "low": 90,
                "close": 100,
                "volume": 1000,
            }])),
        );
    }

    #[backend_test_macros::database_test]
    async fn list_bars_with_invalid_params_returns_400(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;

        let cases = [
            (
                "empty_instrument_id",
                "?instrument_id=",
                serde_json::json!({ "error": "instrument_id must not be empty" }),
            ),
            (
                "invalid_timeframe",
                "?instrument_id=7203&timeframe=5m",
                serde_json::json!({ "error": "invalid timeframe: 5m. valid values: [\"1d\"]" }),
            ),
        ];

        for (name, query, expected_body) in cases {
            let response = server.get(&format!("/api/bars{query}")).await;
            assert_eq!(
                (response.status_code(), response.json::<serde_json::Value>()),
                (StatusCode::BAD_REQUEST, expected_body),
                "case '{name}'",
            );
        }
    }

    #[backend_test_macros::database_test]
    async fn list_bars_returns_empty_when_no_data(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;

        let response = server.get("/api/bars?instrument_id=9999").await;
        assert_response_eq(&response, StatusCode::OK, Some(serde_json::json!([])));
    }

    #[backend_test_macros::database_test]
    async fn list_bars_with_date_range_filters_correctly(db: gateway_postgres::DatabaseHandle) {
        let (db, server) = create_test_server_with_db(db).await;
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
        assert_response_eq(
            &response,
            StatusCode::OK,
            Some(serde_json::json!([{
                "instrument_id": "7203",
                "timeframe": "1d",
                "timestamp": "2025-01-07T00:00:00Z",
                "open": 105,
                "high": 115,
                "low": 95,
                "close": 105,
                "volume": 1000,
            }])),
        );
    }
}
