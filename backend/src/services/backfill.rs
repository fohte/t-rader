use chrono::{Duration, Utc};
use sea_orm::DatabaseConnection;

use crate::data_provider::{DataProvider, DateRange};
use crate::models::Timeframe;
use crate::repositories::bars::upsert_bars;

// J-Quants Free プランのデータ取得可能期間:
// 12 週間前 ~ 2 年 12 週間前
// https://jpx.gitbook.io/j-quants-ja/outline/data-spec
const JQUANTS_FREE_PLAN_OFFSET_WEEKS: i64 = 12;
const JQUANTS_FREE_PLAN_MAX_HISTORY_YEARS: i64 = 2;

/// 指定銘柄の日足データを J-Quants Free プランの取得可能期間分バックフィルする
///
/// Free プランでは 12 週間前 ~ 2 年 12 週間前の範囲のみ取得可能。
/// バックグラウンドタスクとして呼ばれるため、エラー時はログ出力のみで呼び出し元には返さない。
pub async fn backfill_daily_bars(
    db: &DatabaseConnection,
    data_provider: &impl DataProvider,
    instrument_id: &str,
) {
    let today = Utc::now().date_naive();
    // J-Quants Free プランのデータ取得可能範囲にクランプする
    let to = today - Duration::weeks(JQUANTS_FREE_PLAN_OFFSET_WEEKS);
    let from = to - Duration::weeks(JQUANTS_FREE_PLAN_MAX_HISTORY_YEARS * 52);

    let range = DateRange { from, to };

    let bars = match data_provider.fetch_daily_bars(instrument_id, &range).await {
        Ok(bars) => bars,
        Err(e) => {
            tracing::error!(
                instrument_id,
                error = %e,
                "日足データの取得に失敗しました"
            );
            return;
        }
    };

    if bars.is_empty() {
        tracing::info!(instrument_id, "バックフィル対象のデータがありません");
        return;
    }

    // 日足データのみであることを確認
    let daily_bars: Vec<_> = bars
        .into_iter()
        .filter(|b| b.timeframe == Timeframe::Daily)
        .collect();

    let bar_count = daily_bars.len();

    if let Err(e) = upsert_bars(db, daily_bars).await {
        tracing::error!(
            instrument_id,
            error = %e,
            "日足データの保存に失敗しました"
        );
        return;
    }

    tracing::info!(
        instrument_id,
        bar_count,
        "日足データのバックフィルが完了しました"
    );
}

#[cfg(test)]
mod tests {
    use chrono::{Duration, NaiveDate, TimeZone, Utc};
    use rstest::rstest;
    use rust_decimal::Decimal;
    use sqlx::PgPool;

    use super::*;
    use crate::data_provider::{DataProviderError, DateRange};
    use crate::models::instrument::{Instrument, Market};
    use crate::models::{Bar, Timeframe};
    use crate::testing::create_test_db;

    // --- テスト用モック ---

    /// テスト用のモックデータプロバイダー
    struct MockProvider {
        bars: Vec<Bar>,
        instruments: Vec<Instrument>,
    }

    impl MockProvider {
        fn new() -> Self {
            Self {
                bars: Vec::new(),
                instruments: Vec::new(),
            }
        }

        fn with_bars(mut self, bars: Vec<Bar>) -> Self {
            self.bars = bars;
            self
        }

        fn with_instruments(mut self, instruments: Vec<Instrument>) -> Self {
            self.instruments = instruments;
            self
        }
    }

    impl DataProvider for MockProvider {
        async fn fetch_daily_bars(
            &self,
            instrument_id: &str,
            range: &DateRange,
        ) -> Result<Vec<Bar>, DataProviderError> {
            let exists = self.instruments.iter().any(|i| i.id == instrument_id);
            if !exists {
                return Err(DataProviderError::NotFound(format!(
                    "instrument '{instrument_id}' not found"
                )));
            }

            let from_dt =
                Utc.from_utc_datetime(&range.from.and_hms_opt(0, 0, 0).unwrap_or_default());
            let to_exclusive = range.to.succ_opt().unwrap_or(range.to);
            let to_dt =
                Utc.from_utc_datetime(&to_exclusive.and_hms_opt(0, 0, 0).unwrap_or_default());

            let mut bars: Vec<Bar> = self
                .bars
                .iter()
                .filter(|b| {
                    b.instrument_id == instrument_id
                        && b.timestamp >= from_dt
                        && b.timestamp < to_dt
                })
                .cloned()
                .collect();

            bars.sort_by_key(|b| b.timestamp);
            Ok(bars)
        }

        async fn fetch_instrument(
            &self,
            instrument_id: &str,
        ) -> Result<Instrument, DataProviderError> {
            self.instruments
                .iter()
                .find(|i| i.id == instrument_id)
                .cloned()
                .ok_or_else(|| {
                    DataProviderError::NotFound(format!("instrument '{instrument_id}' not found"))
                })
        }
    }

    // --- テスト用ヘルパー ---

    fn make_bar(instrument_id: &str, date: NaiveDate, close: i64) -> Bar {
        let timestamp = Utc.from_utc_datetime(&date.and_hms_opt(0, 0, 0).unwrap_or_default());
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

    fn sample_instrument(id: &str) -> Instrument {
        Instrument {
            id: id.to_string(),
            name: format!("Test {id}"),
            market: Market::Tse,
            sector: None,
        }
    }

    /// テスト用 instrument を DB に挿入する
    async fn insert_test_instrument(db: &DatabaseConnection, id: &str) {
        use crate::entities::instruments;
        use sea_orm::sea_query::OnConflict;
        use sea_orm::{EntityTrait, Set};

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

    // --- テスト ---

    #[sqlx::test(migrations = false)]
    async fn backfill_saves_bars_to_db(pool: PgPool) {
        let db = create_test_db(pool).await;
        insert_test_instrument(&db, "7203").await;

        // backfill_daily_bars と同じロジックで日付範囲を決定する
        let today = Utc::now().date_naive();
        let to = today - Duration::weeks(super::JQUANTS_FREE_PLAN_OFFSET_WEEKS);
        let bars = vec![
            make_bar("7203", to - Duration::days(2), 100),
            make_bar("7203", to - Duration::days(1), 105),
        ];

        let provider = MockProvider::new()
            .with_instruments(vec![sample_instrument("7203")])
            .with_bars(bars);

        backfill_daily_bars(&db, &provider, "7203").await;

        // DB にデータが保存されたことを確認
        use crate::repositories::bars::{BarsQuery, find_bars};
        let result = find_bars(
            &db,
            BarsQuery {
                instrument_id: "7203".to_string(),
                timeframe: "1d".to_string(),
                from: None,
                to: None,
            },
        )
        .await
        .expect("find_bars failed");

        assert_eq!(result.len(), 2);
    }

    #[sqlx::test(migrations = false)]
    async fn backfill_handles_empty_response(pool: PgPool) {
        let db = create_test_db(pool).await;

        // 銘柄は存在するがバーデータなし
        let provider = MockProvider::new().with_instruments(vec![sample_instrument("9999")]);

        // パニックせずに正常終了すること
        backfill_daily_bars(&db, &provider, "9999").await;
    }

    #[rstest]
    #[tokio::test]
    async fn backfill_handles_provider_error() {
        use sea_orm::{DatabaseBackend, MockDatabase};

        let db = MockDatabase::new(DatabaseBackend::Postgres).into_connection();

        // 銘柄が存在しないプロバイダー → NotFound エラー
        let provider = MockProvider::new();

        // パニックせずに正常終了すること
        backfill_daily_bars(&db, &provider, "99999").await;
    }
}
