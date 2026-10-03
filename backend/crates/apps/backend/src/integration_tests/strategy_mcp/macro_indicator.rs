#[cfg(test)]
mod tests {
    use chrono::NaiveDate;
    use core_application::indicator_observation::{
        IndicatorObservationMetadata, IndicatorObservationRepository,
    };
    use core_domain::IndicatorObservation;
    use rust_decimal::Decimal;

    use super::super::dto::{
        IndicatorObservationDto, ReadMacroIndicatorParams, ReadMacroIndicatorResult,
    };
    use super::super::tests_common::{build_server, insert_strategy};
    use gateway_postgres::{DatabaseHandle, PostgresIndicatorObservationRepository};

    fn date(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).expect("valid date")
    }

    async fn seed_observations(db: &DatabaseHandle) {
        let repository = PostgresIndicatorObservationRepository::new(db.clone());
        repository
            .ensure_indicator(IndicatorObservationMetadata {
                indicator_id: "INDICATOR_TEST".to_string(),
                name: "テスト指標".to_string(),
                kind: "index".to_string(),
            })
            .await
            .expect("seed indicator");
        repository
            .upsert_observations(
                "INDICATOR_TEST",
                vec![
                    IndicatorObservation {
                        date: date(2026, 9, 1),
                        value: Decimal::from(10),
                    },
                    IndicatorObservation {
                        date: date(2026, 9, 3),
                        value: Decimal::from(30),
                    },
                    IndicatorObservation {
                        date: date(2026, 9, 10),
                        value: Decimal::from(100),
                    },
                ],
            )
            .await
            .expect("seed observations");
    }

    #[backend_test_macros::database_test]
    async fn returns_observations_in_date_range_oldest_first(db: DatabaseHandle) {
        seed_observations(&db).await;
        let strategy_id = insert_strategy(&db, "sample").await;

        let result = build_server(db)
            .read_macro_indicator(
                strategy_id,
                ReadMacroIndicatorParams {
                    indicator_id: "INDICATOR_TEST".to_string(),
                    from: date(2026, 9, 1),
                    to: date(2026, 9, 5),
                },
            )
            .await
            .expect("read_macro_indicator");

        assert_eq!(
            result,
            ReadMacroIndicatorResult {
                indicator_id: "INDICATOR_TEST".to_string(),
                observations: vec![
                    IndicatorObservationDto {
                        date: date(2026, 9, 1),
                        value: 10.0,
                    },
                    IndicatorObservationDto {
                        date: date(2026, 9, 3),
                        value: 30.0,
                    },
                ],
            },
        );
    }

    #[backend_test_macros::database_test]
    async fn returns_empty_when_no_observation_in_range(db: DatabaseHandle) {
        seed_observations(&db).await;
        let strategy_id = insert_strategy(&db, "sample").await;

        let result = build_server(db)
            .read_macro_indicator(
                strategy_id,
                ReadMacroIndicatorParams {
                    indicator_id: "INDICATOR_TEST".to_string(),
                    from: date(2026, 1, 1),
                    to: date(2026, 1, 31),
                },
            )
            .await
            .expect("read_macro_indicator");

        assert_eq!(
            result,
            ReadMacroIndicatorResult {
                indicator_id: "INDICATOR_TEST".to_string(),
                observations: vec![],
            },
        );
    }

    #[backend_test_macros::database_test]
    async fn rejects_empty_indicator_id(db: DatabaseHandle) {
        let strategy_id = insert_strategy(&db, "sample").await;
        let err = build_server(db)
            .read_macro_indicator(
                strategy_id,
                ReadMacroIndicatorParams {
                    indicator_id: "   ".to_string(),
                    from: date(2026, 1, 1),
                    to: date(2026, 1, 31),
                },
            )
            .await
            .expect_err("empty indicator_id should be rejected");

        assert_eq!(
            err,
            rmcp::ErrorData::invalid_params("indicator_id must not be empty", None),
        );
    }

    #[backend_test_macros::database_test]
    async fn rejects_from_after_to(db: DatabaseHandle) {
        let strategy_id = insert_strategy(&db, "sample").await;
        let err = build_server(db)
            .read_macro_indicator(
                strategy_id,
                ReadMacroIndicatorParams {
                    indicator_id: "INDICATOR_TEST".to_string(),
                    from: date(2026, 9, 10),
                    to: date(2026, 9, 1),
                },
            )
            .await
            .expect_err("from after to should be rejected");

        assert_eq!(
            err,
            rmcp::ErrorData::invalid_params("from must be on or before to", None),
        );
    }
}
