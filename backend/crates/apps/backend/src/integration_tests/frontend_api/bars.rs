#[cfg(test)]
mod tests {
    use super::super::assert_response_eq;
    use crate::testing::{create_test_server, create_test_server_with_db};
    use axum::http::StatusCode;
    use chrono::{NaiveDate, TimeZone, Utc};
    use core_domain::bar::{Bar, Timeframe};
    use gateway_postgres::entities::{instruments, minute_bars};
    use gateway_postgres::repositories;
    use rstest::{fixture, rstest};
    use rust_decimal::Decimal;
    use sea_orm::sea_query::OnConflict;
    use sea_orm::{EntityTrait, Set};

    /// テスト用の instrument を DB に挿入する
    async fn insert_test_instrument(db: &impl sea_orm::ConnectionTrait, id: &str) {
        insert_test_instrument_with_market(db, id, "TSE").await;
    }

    async fn insert_test_instrument_with_market(
        db: &impl sea_orm::ConnectionTrait,
        id: &str,
        market: &str,
    ) {
        instruments::Entity::insert(instruments::ActiveModel {
            id: Set(id.to_string()),
            name: Set(format!("Test {id}")),
            market: Set(market.to_string()),
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

    struct TestMinuteBar {
        hour: u32,
        minute: u32,
        open: i64,
        high: i64,
        low: i64,
        close: i64,
        volume: i64,
    }

    fn make_test_minute_bar(instrument_id: &str, bar: TestMinuteBar) -> minute_bars::ActiveModel {
        let timestamp = NaiveDate::from_ymd_opt(2034, 1, 2)
            .and_then(|date| date.and_hms_opt(bar.hour, bar.minute, 0))
            .map(|datetime| Utc.from_utc_datetime(&datetime).fixed_offset())
            .expect("valid minute bar timestamp");
        minute_bars::ActiveModel {
            instrument_id: Set(instrument_id.to_string()),
            timestamp: Set(timestamp),
            open: Set(Decimal::new(bar.open, 0)),
            high: Set(Decimal::new(bar.high, 0)),
            low: Set(Decimal::new(bar.low, 0)),
            close: Set(Decimal::new(bar.close, 0)),
            volume: Set(bar.volume),
        }
    }

    async fn insert_test_minute_bars(db: &impl sea_orm::ConnectionTrait, id: &str) {
        minute_bars::Entity::insert_many([
            make_test_minute_bar(
                id,
                TestMinuteBar {
                    hour: 14,
                    minute: 30,
                    open: 100,
                    high: 105,
                    low: 99,
                    close: 101,
                    volume: 10,
                },
            ),
            make_test_minute_bar(
                id,
                TestMinuteBar {
                    hour: 14,
                    minute: 31,
                    open: 101,
                    high: 104,
                    low: 100,
                    close: 102,
                    volume: 20,
                },
            ),
            make_test_minute_bar(
                id,
                TestMinuteBar {
                    hour: 14,
                    minute: 35,
                    open: 103,
                    high: 109,
                    low: 102,
                    close: 108,
                    volume: 30,
                },
            ),
            make_test_minute_bar(
                id,
                TestMinuteBar {
                    hour: 14,
                    minute: 39,
                    open: 108,
                    high: 110,
                    low: 107,
                    close: 109,
                    volume: 40,
                },
            ),
        ])
        .exec_without_returning(db)
        .await
        .expect("failed to insert minute bars");
    }

    #[fixture]
    async fn intraday_bars_context() -> (gateway_postgres::DatabaseHandle, axum_test::TestServer) {
        let db = gateway_postgres::test_support::create_test_transaction().await;
        let (db, server) = create_test_server_with_db(db).await;
        insert_test_instrument_with_market(&db, "DEMO-US-ALPHA", "US").await;
        insert_test_minute_bars(&db, "DEMO-US-ALPHA").await;
        (db, server)
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
            adjustment_factor: Decimal::ONE,
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
                "?instrument_id=demo-code&timeframe=3m",
                serde_json::json!({ "error": "invalid timeframe: 3m. valid values: [\"1d\", \"1m\", \"5m\", \"15m\", \"1h\", \"4h\"]" }),
            ),
            (
                "invalid_intraday_datetime",
                "?instrument_id=demo-code&timeframe=1m&from=2025-01-07",
                serde_json::json!({ "error": "from must be an RFC 3339 datetime for intraday timeframes" }),
            ),
            (
                "reversed_intraday_range",
                "?instrument_id=demo-code&timeframe=1m&from=2025-01-07T09:31:00Z&to=2025-01-07T09:30:00Z",
                serde_json::json!({ "error": "from must be earlier than or equal to to" }),
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
        insert_test_instrument(&db, "demo-code").await;

        let bars = vec![
            make_test_bar(
                "demo-code",
                NaiveDate::from_ymd_opt(2025, 1, 6).expect("invalid date"),
                100,
            ),
            make_test_bar(
                "demo-code",
                NaiveDate::from_ymd_opt(2025, 1, 7).expect("invalid date"),
                105,
            ),
            make_test_bar(
                "demo-code",
                NaiveDate::from_ymd_opt(2025, 1, 8).expect("invalid date"),
                103,
            ),
        ];
        repositories::bars::upsert_bars(&db, bars)
            .await
            .expect("upsert failed");

        let response = server
            .get("/api/bars?instrument_id=demo-code&from=2025-01-07&to=2025-01-07")
            .await;
        assert_response_eq(
            &response,
            StatusCode::OK,
            Some(serde_json::json!([{
                "instrument_id": "demo-code",
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

    #[rstest]
    #[case::minute("1m", serde_json::json!([
        {"instrument_id":"DEMO-US-ALPHA","timeframe":"1m","timestamp":"2034-01-02T14:30:00Z","open":100,"high":105,"low":99,"close":101,"volume":10},
        {"instrument_id":"DEMO-US-ALPHA","timeframe":"1m","timestamp":"2034-01-02T14:31:00Z","open":101,"high":104,"low":100,"close":102,"volume":20},
        {"instrument_id":"DEMO-US-ALPHA","timeframe":"1m","timestamp":"2034-01-02T14:35:00Z","open":103,"high":109,"low":102,"close":108,"volume":30},
        {"instrument_id":"DEMO-US-ALPHA","timeframe":"1m","timestamp":"2034-01-02T14:39:00Z","open":108,"high":110,"low":107,"close":109,"volume":40}
    ]))]
    #[case::five_minutes("5m", serde_json::json!([
        {"instrument_id":"DEMO-US-ALPHA","timeframe":"5m","timestamp":"2034-01-02T14:30:00Z","open":100,"high":105,"low":99,"close":102,"volume":30},
        {"instrument_id":"DEMO-US-ALPHA","timeframe":"5m","timestamp":"2034-01-02T14:35:00Z","open":103,"high":110,"low":102,"close":109,"volume":70}
    ]))]
    #[case::fifteen_minutes("15m", serde_json::json!([
        {"instrument_id":"DEMO-US-ALPHA","timeframe":"15m","timestamp":"2034-01-02T14:30:00Z","open":100,"high":110,"low":99,"close":109,"volume":100}
    ]))]
    #[case::one_hour("1h", serde_json::json!([
        {"instrument_id":"DEMO-US-ALPHA","timeframe":"1h","timestamp":"2034-01-02T14:00:00Z","open":100,"high":110,"low":99,"close":109,"volume":100}
    ]))]
    #[case::four_hours("4h", serde_json::json!([
        {"instrument_id":"DEMO-US-ALPHA","timeframe":"4h","timestamp":"2034-01-02T12:00:00Z","open":100,"high":110,"low":99,"close":109,"volume":100}
    ]))]
    #[tokio::test]
    async fn list_intraday_bars_returns_minute_and_aggregated_timeframes(
        #[future] intraday_bars_context: (gateway_postgres::DatabaseHandle, axum_test::TestServer),
        #[case] timeframe: &str,
        #[case] expected_body: serde_json::Value,
    ) {
        let (_, server) = intraday_bars_context.await;
        let response = server
            .get(&format!(
                "/api/bars?instrument_id=DEMO-US-ALPHA&timeframe={timeframe}&from=2034-01-02T12:00:00Z&to=2034-01-02T14:39:00Z"
            ))
            .await;
        assert_response_eq(&response, StatusCode::OK, Some(expected_body));
    }

    #[rstest]
    #[case::minute("1m", serde_json::json!([
        {"instrument_id":"DEMO-US-ALPHA","timeframe":"1m","timestamp":"2034-01-02T14:35:00Z","open":103,"high":109,"low":102,"close":108,"volume":30}
    ]))]
    #[case::five_minutes("5m", serde_json::json!([
        {"instrument_id":"DEMO-US-ALPHA","timeframe":"5m","timestamp":"2034-01-02T14:35:00Z","open":103,"high":110,"low":102,"close":109,"volume":70}
    ]))]
    #[tokio::test]
    async fn list_intraday_bars_filters_by_inclusive_datetime_range(
        #[future] intraday_bars_context: (gateway_postgres::DatabaseHandle, axum_test::TestServer),
        #[case] timeframe: &str,
        #[case] expected_body: serde_json::Value,
    ) {
        let (_, server) = intraday_bars_context.await;
        let response = server
            .get(&format!(
                "/api/bars?instrument_id=DEMO-US-ALPHA&timeframe={timeframe}&from=2034-01-02T14:35:00Z&to=2034-01-02T14:35:00Z"
            ))
            .await;
        assert_response_eq(&response, StatusCode::OK, Some(expected_body));
    }

    #[rstest]
    #[case::minute("1m", serde_json::json!([
        {"instrument_id":"DEMO-US-ALPHA","timeframe":"1m","timestamp":"2034-01-02T14:30:00Z","open":100,"high":105,"low":99,"close":101,"volume":10},
        {"instrument_id":"DEMO-US-ALPHA","timeframe":"1m","timestamp":"2034-01-02T14:31:00Z","open":101,"high":104,"low":100,"close":102,"volume":20},
        {"instrument_id":"DEMO-US-ALPHA","timeframe":"1m","timestamp":"2034-01-02T14:35:00Z","open":103,"high":109,"low":102,"close":108,"volume":30},
        {"instrument_id":"DEMO-US-ALPHA","timeframe":"1m","timestamp":"2034-01-02T14:39:00Z","open":108,"high":110,"low":107,"close":109,"volume":40}
    ]))]
    #[case::five_minutes("5m", serde_json::json!([
        {"instrument_id":"DEMO-US-ALPHA","timeframe":"5m","timestamp":"2034-01-02T14:30:00Z","open":100,"high":105,"low":99,"close":102,"volume":30},
        {"instrument_id":"DEMO-US-ALPHA","timeframe":"5m","timestamp":"2034-01-02T14:35:00Z","open":103,"high":110,"low":102,"close":109,"volume":70}
    ]))]
    #[tokio::test]
    async fn list_intraday_bars_returns_all_data_without_range(
        #[future] intraday_bars_context: (gateway_postgres::DatabaseHandle, axum_test::TestServer),
        #[case] timeframe: &str,
        #[case] expected_body: serde_json::Value,
    ) {
        let (_, server) = intraday_bars_context.await;
        let response = server
            .get(&format!(
                "/api/bars?instrument_id=DEMO-US-ALPHA&timeframe={timeframe}"
            ))
            .await;
        assert_response_eq(&response, StatusCode::OK, Some(expected_body));
    }

    #[backend_test_macros::database_test]
    async fn list_intraday_bars_returns_empty_for_non_us_instrument(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let (db, server) = create_test_server_with_db(db).await;
        insert_test_instrument(&db, "DEMO-TSE-ALPHA").await;
        insert_test_minute_bars(&db, "DEMO-TSE-ALPHA").await;

        let response = server
            .get("/api/bars?instrument_id=DEMO-TSE-ALPHA&timeframe=1m")
            .await;

        assert_response_eq(&response, StatusCode::OK, Some(serde_json::json!([])));
    }
}
