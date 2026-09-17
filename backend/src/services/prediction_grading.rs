//! 期限到来した予測 (prediction) を日足データで自動採点する。
//!
//! target/benchmark それぞれの base_date・due_date 時点の終値からリターンを計算し、
//! `direction` (outperform/underperform) との整合で的中・不的中を判定する。判定はコードで
//! 決定的に行い、LLM は関与しない。

use std::collections::HashSet;
use std::time::Duration;

use rust_decimal::Decimal;
use sea_orm::ActiveValue::{NotSet, Set};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QuerySelect,
};
use uuid::Uuid;

use crate::entities::{prediction, prediction_grade};
use crate::error::AppError;
use crate::repositories::bars::find_latest_bar_on_or_before;

/// poll task のデフォルト実行間隔。日足の確定を待つだけの処理で緊急性が無いため週次とする。
pub const DEFAULT_INTERVAL: Duration = Duration::from_secs(7 * 24 * 60 * 60);

const DAILY_TIMEFRAME: &str = "1d";

/// 1 サイクル分の採点結果
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct GradingStats {
    pub graded: usize,
}

/// 採点に必要な終値とリターン、的中結果
struct GradedOutcome {
    target_base_close: Decimal,
    target_due_close: Decimal,
    benchmark_base_close: Decimal,
    benchmark_due_close: Decimal,
    target_return: Decimal,
    benchmark_return: Decimal,
    correct: bool,
}

/// `(due_close - base_close) / base_close` を計算する。`base` が 0 の場合は `None`。
fn compute_return(base: Decimal, due: Decimal) -> Option<Decimal> {
    (due - base).checked_div(base)
}

/// 1 件の予測を採点する。終値がまだ揃っていない場合は `Ok(None)` を返し、次回サイクルに
/// 持ち越す。
async fn try_grade(
    db: &DatabaseConnection,
    p: &prediction::Model,
) -> Result<Option<GradedOutcome>, AppError> {
    let due_bday = crate::date_utils::latest_business_day(p.due_date);

    let Some(target_due_bar) =
        find_latest_bar_on_or_before(db, &p.target_stock_id, DAILY_TIMEFRAME, p.due_date).await?
    else {
        tracing::debug!(
            prediction_id = %p.prediction_id,
            "target の due_date 以前のバーが 1 件も無いためスキップします",
        );
        return Ok(None);
    };
    let Some(benchmark_due_bar) =
        find_latest_bar_on_or_before(db, &p.benchmark_stock_id, DAILY_TIMEFRAME, p.due_date)
            .await?
    else {
        tracing::debug!(
            prediction_id = %p.prediction_id,
            "benchmark の due_date 以前のバーが 1 件も無いためスキップします",
        );
        return Ok(None);
    };

    // 「due_date 以前で最新」のバーが due_bday より古い場合、期限日の日足がまだ
    // ingest されていない。古いデータで採点せず次回サイクルに持ち越す。
    if target_due_bar.timestamp.date_naive() != due_bday
        || benchmark_due_bar.timestamp.date_naive() != due_bday
    {
        tracing::debug!(
            prediction_id = %p.prediction_id,
            "due_date の日足がまだ ingest されていないためスキップします",
        );
        return Ok(None);
    }

    // base_date は既に過去の日付であり、当時の終値が今後 ingest されることはないため、
    // due_date と異なり「取得できたバーが最新か」の鮮度チェックは行わない。
    // 非営業日の base_date に対する直近営業日へのフォールバックも意図した挙動。
    let Some(target_base_bar) =
        find_latest_bar_on_or_before(db, &p.target_stock_id, DAILY_TIMEFRAME, p.base_date).await?
    else {
        tracing::debug!(
            prediction_id = %p.prediction_id,
            "target の base_date 以前のバーが 1 件も無いためスキップします",
        );
        return Ok(None);
    };
    let Some(benchmark_base_bar) =
        find_latest_bar_on_or_before(db, &p.benchmark_stock_id, DAILY_TIMEFRAME, p.base_date)
            .await?
    else {
        tracing::debug!(
            prediction_id = %p.prediction_id,
            "benchmark の base_date 以前のバーが 1 件も無いためスキップします",
        );
        return Ok(None);
    };

    let Some(target_return) = compute_return(target_base_bar.close, target_due_bar.close) else {
        tracing::debug!(
            prediction_id = %p.prediction_id,
            "target の base_date 終値が 0 のためリターンを計算できずスキップします",
        );
        return Ok(None);
    };
    let Some(benchmark_return) = compute_return(benchmark_base_bar.close, benchmark_due_bar.close)
    else {
        tracing::debug!(
            prediction_id = %p.prediction_id,
            "benchmark の base_date 終値が 0 のためリターンを計算できずスキップします",
        );
        return Ok(None);
    };

    // direction は DB の CHECK 制約で "outperform" / "underperform" のみが保証される。
    let correct = if p.direction == "outperform" {
        target_return > benchmark_return
    } else {
        target_return < benchmark_return
    };

    Ok(Some(GradedOutcome {
        target_base_close: target_base_bar.close,
        target_due_close: target_due_bar.close,
        benchmark_base_close: benchmark_base_bar.close,
        benchmark_due_close: benchmark_due_bar.close,
        target_return,
        benchmark_return,
        correct,
    }))
}

/// 期限到来済みかつ未採点の予測をまとめて採点する。全戦略横断で対象を取得する
/// (個人利用規模のため全件取得で問題ない)。
pub async fn run_grading_cycle(db: &DatabaseConnection) -> Result<GradingStats, AppError> {
    let today = chrono::Utc::now().date_naive();

    let due_predictions = prediction::Entity::find()
        .filter(prediction::Column::DueDate.lte(today))
        .all(db)
        .await?;
    if due_predictions.is_empty() {
        return Ok(GradingStats::default());
    }

    let graded_ids: HashSet<Uuid> = prediction_grade::Entity::find()
        .select_only()
        .column(prediction_grade::Column::PredictionId)
        .into_tuple::<Uuid>()
        .all(db)
        .await?
        .into_iter()
        .collect();

    let mut stats = GradingStats::default();
    for p in due_predictions {
        if graded_ids.contains(&p.prediction_id) {
            continue;
        }

        match try_grade(db, &p).await {
            Ok(Some(outcome)) => {
                let insert_result = prediction_grade::ActiveModel {
                    prediction_id: Set(p.prediction_id),
                    target_base_close: Set(outcome.target_base_close),
                    target_due_close: Set(outcome.target_due_close),
                    benchmark_base_close: Set(outcome.benchmark_base_close),
                    benchmark_due_close: Set(outcome.benchmark_due_close),
                    target_return: Set(outcome.target_return),
                    benchmark_return: Set(outcome.benchmark_return),
                    correct: Set(outcome.correct),
                    graded_at: NotSet,
                }
                .insert(db)
                .await;
                match insert_result {
                    Ok(_) => stats.graded += 1,
                    Err(err) => {
                        tracing::warn!(
                            error = %err,
                            prediction_id = %p.prediction_id,
                            "failed to save prediction grade; will retry next cycle",
                        );
                    }
                }
            }
            Ok(None) => {}
            Err(err) => {
                tracing::warn!(
                    error = %err,
                    prediction_id = %p.prediction_id,
                    "failed to grade prediction; will retry next cycle",
                );
            }
        }
    }

    Ok(stats)
}

/// poll task を起動する。未採点かつ期限到来済みの予測だけを扱うため、起動直後の即時実行も
/// 含めて何度実行しても結果は同じ (べき等)。
pub fn spawn_poll(db: DatabaseConnection, interval: Duration) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(interval);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            ticker.tick().await;
            match run_grading_cycle(&db).await {
                Ok(stats) => {
                    tracing::debug!(graded = stats.graded, "prediction grading cycle completed");
                }
                Err(err) => tracing::warn!(error = %err, "prediction grading cycle failed"),
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;
    use rstest::rstest;
    use sea_orm::ActiveValue::NotSet;
    use sqlx::PgPool;

    use super::*;
    use crate::entities::instruments;
    use crate::models::Bar;
    use crate::models::bar::Timeframe;
    use crate::repositories::bars::upsert_bars;
    use crate::testing::{create_test_db, insert_test_stock, insert_test_strategy};

    #[rstest]
    #[case::positive_return(Decimal::new(100, 0), Decimal::new(110, 0), Some(Decimal::new(10, 2)))]
    #[case::negative_return(Decimal::new(100, 0), Decimal::new(90, 0), Some(Decimal::new(-10, 2)))]
    #[case::zero_base_is_none(Decimal::new(0, 0), Decimal::new(90, 0), None)]
    fn compute_return_cases(
        #[case] base: Decimal,
        #[case] due: Decimal,
        #[case] expected: Option<Decimal>,
    ) {
        assert_eq!(compute_return(base, due), expected);
    }

    fn date(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).expect("valid date")
    }

    /// `stock` (prediction の FK 先) と `instruments` (bars の FK 先) は別テーブルなので、
    /// 同じ id で両方に行を作る。
    async fn insert_test_target(db: &DatabaseConnection, id: &str, name: &str) {
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
        db: &DatabaseConnection,
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
        db: &DatabaseConnection,
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
        db: &DatabaseConnection,
        target_base: i64,
        target_due: i64,
        benchmark_base: i64,
        benchmark_due: i64,
    ) {
        insert_test_target(db, "1000", "target").await;
        insert_test_target(db, "2000", "benchmark").await;
        upsert_bars(
            db,
            vec![
                bar("1000", date(2026, 6, 15), target_base),
                bar("1000", date(2026, 6, 22), target_due),
                bar("2000", date(2026, 6, 15), benchmark_base),
                bar("2000", date(2026, 6, 22), benchmark_due),
            ],
        )
        .await
        .expect("seed bars");
    }

    #[sqlx::test(migrations = false)]
    async fn grades_outperform_prediction_as_correct(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_id = insert_test_strategy(&db, "test").await;
        seed_scenario(&db, 100, 120, 100, 110).await;
        let prediction_id = insert_prediction(
            &db,
            strategy_id,
            "1000",
            "2000",
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

    #[sqlx::test(migrations = false)]
    async fn grades_underperform_prediction_as_incorrect(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_id = insert_test_strategy(&db, "test").await;
        // target が benchmark を上回っているので underperform の予測は外れる。
        seed_scenario(&db, 100, 120, 100, 110).await;
        let prediction_id = insert_prediction(
            &db,
            strategy_id,
            "1000",
            "2000",
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

    #[sqlx::test(migrations = false)]
    async fn skips_when_due_date_bar_is_stale(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_id = insert_test_strategy(&db, "test").await;
        insert_test_target(&db, "1000", "target").await;
        insert_test_target(&db, "2000", "benchmark").await;
        // due_date (2026-06-22, 月・平日) の日足がまだ無く、直前 (2026-06-19) までしか
        // ingest されていない状態を再現する。
        upsert_bars(
            &db,
            vec![
                bar("1000", date(2026, 6, 15), 100),
                bar("1000", date(2026, 6, 19), 120),
                bar("2000", date(2026, 6, 15), 100),
                bar("2000", date(2026, 6, 19), 110),
            ],
        )
        .await
        .expect("seed bars");
        let prediction_id = insert_prediction(
            &db,
            strategy_id,
            "1000",
            "2000",
            "outperform",
            date(2026, 6, 15),
            date(2026, 6, 22),
        )
        .await;

        let stats = run_grading_cycle(&db).await.expect("cycle ok");

        assert_eq!(stats, GradingStats { graded: 0 });
        assert_eq!(find_grade(&db, prediction_id).await, None);
    }

    #[sqlx::test(migrations = false)]
    async fn skips_when_base_date_bar_is_missing(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_id = insert_test_strategy(&db, "test").await;
        insert_test_target(&db, "1000", "target").await;
        insert_test_target(&db, "2000", "benchmark").await;
        // due 側は揃っているが、base_date 以前のバーが存在しない。
        upsert_bars(
            &db,
            vec![
                bar("1000", date(2026, 6, 22), 120),
                bar("2000", date(2026, 6, 22), 110),
            ],
        )
        .await
        .expect("seed bars");
        let prediction_id = insert_prediction(
            &db,
            strategy_id,
            "1000",
            "2000",
            "outperform",
            date(2026, 6, 15),
            date(2026, 6, 22),
        )
        .await;

        let stats = run_grading_cycle(&db).await.expect("cycle ok");

        assert_eq!(stats, GradingStats { graded: 0 });
        assert_eq!(find_grade(&db, prediction_id).await, None);
    }

    #[sqlx::test(migrations = false)]
    async fn already_graded_prediction_is_not_regraded(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_id = insert_test_strategy(&db, "test").await;
        seed_scenario(&db, 100, 120, 100, 110).await;
        let prediction_id = insert_prediction(
            &db,
            strategy_id,
            "1000",
            "2000",
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

    #[sqlx::test(migrations = false)]
    async fn one_skipped_prediction_does_not_block_others_in_same_cycle(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_id = insert_test_strategy(&db, "test").await;
        seed_scenario(&db, 100, 120, 100, 110).await;
        let gradable_id = insert_prediction(
            &db,
            strategy_id,
            "1000",
            "2000",
            "outperform",
            date(2026, 6, 15),
            date(2026, 6, 22),
        )
        .await;

        // 同じ target/benchmark だが base_date のバーが無いため採点できない予測を混在させる。
        insert_prediction(
            &db,
            strategy_id,
            "1000",
            "2000",
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
