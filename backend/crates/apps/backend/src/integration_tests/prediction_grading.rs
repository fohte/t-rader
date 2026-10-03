mod tests {
    use crate::testing::{insert_test_stock, insert_test_strategy};
    use chrono::NaiveDate;
    use core_application::prediction::{GradingStats, PredictionUseCaseError};
    use core_domain::bar::{Bar, Timeframe};
    use gateway_postgres::DatabaseHandle;
    use gateway_postgres::entities::{instruments, prediction, prediction_grade};
    use gateway_postgres::repositories::bars::upsert_bars;
    use rust_decimal::Decimal;
    use sea_orm::ActiveValue::{NotSet, Set};
    use sea_orm::{ActiveModelTrait, EntityTrait, QuerySelect};
    use uuid::Uuid;

    async fn run_grading_cycle(
        db: &DatabaseHandle,
    ) -> Result<GradingStats, PredictionUseCaseError> {
        let use_cases = crate::services::use_cases::build_use_cases(db.clone());
        use_cases.predictions().grade_due_today().await
    }

    fn date(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).expect("valid date")
    }

    /// `stock` (prediction の FK 先) と `instruments` (bars の FK 先) は別テーブルなので、
    /// 同じ id で両方に行を作る。
    async fn insert_test_target(db: &impl sea_orm::ConnectionTrait, id: &str, name: &str) {
        insert_test_stock(db, id, name).await;
        instruments::Entity::insert(instruments::ActiveModel {
            id: Set(id.to_string()),
            name: Set(name.to_string()),
            market: Set("TSE".to_string()),
            sector: Set(None),
        })
        .exec(db)
        .await
        .expect("insert test instrument");
    }

    fn bar(instrument_id: &str, date: NaiveDate, close: i64) -> Bar {
        let timestamp = date
            .and_hms_opt(0, 0, 0)
            .map(|dt| dt.and_utc())
            .expect("valid datetime");
        Bar {
            instrument_id: instrument_id.to_string(),
            timeframe: Timeframe::Daily,
            timestamp,
            open: Decimal::new(close, 0),
            high: Decimal::new(close, 0),
            low: Decimal::new(close, 0),
            close: Decimal::new(close, 0),
            volume: 1000,
        }
    }

    /// テスト用の予測を 1 件挿入する。`base_date`/`due_date` 以外は固定値。
    async fn insert_prediction(
        db: &impl sea_orm::ConnectionTrait,
        strategy_id: Uuid,
        target_stock_id: &str,
        benchmark_stock_id: &str,
        direction: &str,
        base_date: NaiveDate,
        due_date: NaiveDate,
    ) -> Uuid {
        let prediction_id = Uuid::new_v4();
        prediction::ActiveModel {
            prediction_id: Set(prediction_id),
            strategy_id: Set(strategy_id),
            note_id: Set(None),
            target_stock_id: Set(target_stock_id.to_string()),
            benchmark_stock_id: Set(benchmark_stock_id.to_string()),
            direction: Set(direction.to_string()),
            probability: Set(Decimal::new(70, 2)),
            base_date: Set(base_date),
            due_date: Set(due_date),
            created_at: NotSet,
        }
        .insert(db)
        .await
        .expect("insert test prediction");
        prediction_id
    }

    async fn find_grade(
        db: &impl sea_orm::ConnectionTrait,
        prediction_id: Uuid,
    ) -> Option<prediction_grade::Model> {
        prediction_grade::Entity::find_by_id(prediction_id)
            .one(db)
            .await
            .expect("query prediction_grade")
    }

    /// 2026-06-15 (月・平日) を base、2026-06-22 (月・平日) を due とする共通シナリオ。
    /// target/benchmark ともに base/due の日足が揃っている前提を作る。
    async fn seed_scenario(
        db: &impl sea_orm::ConnectionTrait,
        target_base: i64,
        target_due: i64,
        benchmark_base: i64,
        benchmark_due: i64,
    ) {
        insert_test_target(db, "SAMPLE_TARGET", "target").await;
        insert_test_target(db, "SAMPLE_BENCHMARK", "benchmark").await;
        upsert_bars(
            db,
            vec![
                bar("SAMPLE_TARGET", date(2026, 6, 15), target_base),
                bar("SAMPLE_TARGET", date(2026, 6, 22), target_due),
                bar("SAMPLE_BENCHMARK", date(2026, 6, 15), benchmark_base),
                bar("SAMPLE_BENCHMARK", date(2026, 6, 22), benchmark_due),
            ],
        )
        .await
        .expect("seed bars");
    }

    #[backend_test_macros::database_test]
    async fn grades_outperform_prediction_as_correct(db: gateway_postgres::DatabaseHandle) {
        let strategy_id = insert_test_strategy(&db, "test").await;
        seed_scenario(&db, 100, 120, 100, 110).await;
        let prediction_id = insert_prediction(
            &db,
            strategy_id,
            "SAMPLE_TARGET",
            "SAMPLE_BENCHMARK",
            "outperform",
            date(2026, 6, 15),
            date(2026, 6, 22),
        )
        .await;

        let stats = run_grading_cycle(&db).await.expect("cycle ok");

        assert_eq!(stats, GradingStats { graded: 1 });
        let grade = find_grade(&db, prediction_id)
            .await
            .expect("grade row exists");
        assert_eq!(
            grade,
            prediction_grade::Model {
                prediction_id,
                target_base_close: Decimal::new(100, 0),
                target_due_close: Decimal::new(120, 0),
                benchmark_base_close: Decimal::new(100, 0),
                benchmark_due_close: Decimal::new(110, 0),
                target_return: Decimal::new(20, 2),
                benchmark_return: Decimal::new(10, 2),
                correct: true,
                graded_at: grade.graded_at,
            }
        );
    }

    #[backend_test_macros::database_test]
    async fn grades_underperform_prediction_as_incorrect(db: gateway_postgres::DatabaseHandle) {
        let strategy_id = insert_test_strategy(&db, "test").await;
        // target が benchmark を上回っているので underperform の予測は外れる。
        seed_scenario(&db, 100, 120, 100, 110).await;
        let prediction_id = insert_prediction(
            &db,
            strategy_id,
            "SAMPLE_TARGET",
            "SAMPLE_BENCHMARK",
            "underperform",
            date(2026, 6, 15),
            date(2026, 6, 22),
        )
        .await;

        let stats = run_grading_cycle(&db).await.expect("cycle ok");

        assert_eq!(stats, GradingStats { graded: 1 });
        let grade = find_grade(&db, prediction_id)
            .await
            .expect("grade row exists");
        assert_eq!(
            grade,
            prediction_grade::Model {
                prediction_id,
                target_base_close: Decimal::new(100, 0),
                target_due_close: Decimal::new(120, 0),
                benchmark_base_close: Decimal::new(100, 0),
                benchmark_due_close: Decimal::new(110, 0),
                target_return: Decimal::new(20, 2),
                benchmark_return: Decimal::new(10, 2),
                correct: false,
                graded_at: grade.graded_at,
            }
        );
    }

    #[backend_test_macros::database_test]
    async fn skips_when_due_date_bar_is_stale(db: gateway_postgres::DatabaseHandle) {
        let strategy_id = insert_test_strategy(&db, "test").await;
        insert_test_target(&db, "SAMPLE_TARGET", "target").await;
        insert_test_target(&db, "SAMPLE_BENCHMARK", "benchmark").await;
        // due_date (2026-06-22, 月・平日) の日足がまだ無く、直前 (2026-06-19) までしか
        // ingest されていない状態を再現する。
        upsert_bars(
            &db,
            vec![
                bar("SAMPLE_TARGET", date(2026, 6, 15), 100),
                bar("SAMPLE_TARGET", date(2026, 6, 19), 120),
                bar("SAMPLE_BENCHMARK", date(2026, 6, 15), 100),
                bar("SAMPLE_BENCHMARK", date(2026, 6, 19), 110),
            ],
        )
        .await
        .expect("seed bars");
        let prediction_id = insert_prediction(
            &db,
            strategy_id,
            "SAMPLE_TARGET",
            "SAMPLE_BENCHMARK",
            "outperform",
            date(2026, 6, 15),
            date(2026, 6, 22),
        )
        .await;

        let stats = run_grading_cycle(&db).await.expect("cycle ok");

        assert_eq!(stats, GradingStats { graded: 0 });
        assert_eq!(find_grade(&db, prediction_id).await, None);
    }

    #[backend_test_macros::database_test]
    async fn skips_when_base_date_bar_is_missing(db: gateway_postgres::DatabaseHandle) {
        let strategy_id = insert_test_strategy(&db, "test").await;
        insert_test_target(&db, "SAMPLE_TARGET", "target").await;
        insert_test_target(&db, "SAMPLE_BENCHMARK", "benchmark").await;
        // due 側は揃っているが、base_date 以前のバーが存在しない。
        upsert_bars(
            &db,
            vec![
                bar("SAMPLE_TARGET", date(2026, 6, 22), 120),
                bar("SAMPLE_BENCHMARK", date(2026, 6, 22), 110),
            ],
        )
        .await
        .expect("seed bars");
        let prediction_id = insert_prediction(
            &db,
            strategy_id,
            "SAMPLE_TARGET",
            "SAMPLE_BENCHMARK",
            "outperform",
            date(2026, 6, 15),
            date(2026, 6, 22),
        )
        .await;

        let stats = run_grading_cycle(&db).await.expect("cycle ok");

        assert_eq!(stats, GradingStats { graded: 0 });
        assert_eq!(find_grade(&db, prediction_id).await, None);
    }

    #[backend_test_macros::database_test]
    async fn skips_when_base_close_is_zero(db: gateway_postgres::DatabaseHandle) {
        let strategy_id = insert_test_strategy(&db, "test").await;
        seed_scenario(&db, 0, 120, 100, 110).await;
        let prediction_id = insert_prediction(
            &db,
            strategy_id,
            "SAMPLE_TARGET",
            "SAMPLE_BENCHMARK",
            "outperform",
            date(2026, 6, 15),
            date(2026, 6, 22),
        )
        .await;

        let result = (
            run_grading_cycle(&db).await.expect("cycle ok"),
            find_grade(&db, prediction_id).await,
        );

        assert_eq!(result, (GradingStats::default(), None));
    }

    #[backend_test_macros::database_test]
    async fn already_graded_prediction_is_not_regraded(db: gateway_postgres::DatabaseHandle) {
        let strategy_id = insert_test_strategy(&db, "test").await;
        seed_scenario(&db, 100, 120, 100, 110).await;
        let prediction_id = insert_prediction(
            &db,
            strategy_id,
            "SAMPLE_TARGET",
            "SAMPLE_BENCHMARK",
            "outperform",
            date(2026, 6, 15),
            date(2026, 6, 22),
        )
        .await;

        let first = run_grading_cycle(&db).await.expect("cycle ok");
        assert_eq!(first, GradingStats { graded: 1 });
        let grade_after_first = find_grade(&db, prediction_id)
            .await
            .expect("grade row exists");

        let second = run_grading_cycle(&db).await.expect("cycle ok");
        assert_eq!(second, GradingStats { graded: 0 });
        let grade_after_second = find_grade(&db, prediction_id)
            .await
            .expect("grade row exists");
        assert_eq!(grade_after_first, grade_after_second);
    }

    #[backend_test_macros::database_test]
    async fn one_skipped_prediction_does_not_block_others_in_same_cycle(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let strategy_id = insert_test_strategy(&db, "test").await;
        seed_scenario(&db, 100, 120, 100, 110).await;
        let gradable_id = insert_prediction(
            &db,
            strategy_id,
            "SAMPLE_TARGET",
            "SAMPLE_BENCHMARK",
            "outperform",
            date(2026, 6, 15),
            date(2026, 6, 22),
        )
        .await;

        // 同じ target/benchmark だが base_date のバーが無いため採点できない予測を混在させる。
        insert_prediction(
            &db,
            strategy_id,
            "SAMPLE_TARGET",
            "SAMPLE_BENCHMARK",
            "outperform",
            date(2026, 1, 1),
            date(2026, 6, 22),
        )
        .await;

        let stats = run_grading_cycle(&db).await.expect("cycle ok");

        assert_eq!(stats, GradingStats { graded: 1 });
        let graded_ids: Vec<Uuid> = prediction_grade::Entity::find()
            .select_only()
            .column(prediction_grade::Column::PredictionId)
            .into_tuple::<Uuid>()
            .all(&db)
            .await
            .expect("query prediction_grade");
        assert_eq!(graded_ids, vec![gradable_id]);
    }
}
