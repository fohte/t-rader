use chrono::{Duration, NaiveDate, Utc};
use sea_orm::DatabaseConnection;

use crate::data_provider::{DataProvider, DateRange};
use crate::models::Timeframe;
use crate::models::jquants_plan::PROBE_MAX_HISTORY_DAYS;
use crate::repositories::bars::upsert_bars;

/// 価格データを取得可能な最新日 (未検出時は today、検出済み時は契約上限日) を返す。
/// 保有時価の評価上限 (`market_price::fetch_latest_prices`) もこの関数を経由するため、
/// 取得側と評価側で上限がずれることはない。
pub(crate) fn latest_fetchable_date(
    today: NaiveDate,
    known_range: Option<(NaiveDate, NaiveDate)>,
) -> NaiveDate {
    known_range.map_or_else(
        || {
            tracing::warn!("契約範囲が未検出のため、上限を today として扱い取得を試みます");
            today
        },
        |(_, to)| to,
    )
}

/// 指定銘柄の日足データをバックフィルする。
///
/// 契約範囲が検出済みならその範囲を、未検出ならプローブ範囲を取得する。
/// バックグラウンドタスクとして呼ばれるため、エラー時はログ出力のみで呼び出し元には返さない。
pub async fn backfill_daily_bars(
    db: &DatabaseConnection,
    data_provider: &impl DataProvider,
    instrument_id: &str,
) {
    let today = Utc::now().date_naive();
    let range = match data_provider.known_fetchable_range() {
        Some((from, to)) => DateRange { from, to },
        None => DateRange {
            from: today - Duration::days(PROBE_MAX_HISTORY_DAYS),
            to: today,
        },
    };

    let fetch_result = data_provider.fetch_daily_bars(instrument_id, &range).await;

    // 未設定の間だけ、検出済みの契約範囲を初回のプランとして推定・永続化する。
    // fetch の成否に関わらず (400 検出はリトライ前に記録されるため) 呼んでよい。
    // backfill 全体を失敗させたくないため warn ログのみに留める。
    if let Err(e) = data_provider.persist_inferred_range_if_needed(db).await {
        tracing::warn!(error = %e, "契約プランの推定結果の永続化に失敗しました");
    }

    let bars = match fetch_result {
        Ok(bars) => bars,
        Err(e) => {
            tracing::error!(instrument_id, error = %e, "日足データの取得に失敗しました");
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
    use crate::models::instrument::{Instrument, Market};
    use crate::models::{Bar, Timeframe};
    use crate::testing::{MockProvider, create_test_db};

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

    async fn find_all_bars(
        db: &DatabaseConnection,
        instrument_id: &str,
    ) -> Vec<crate::entities::bars::Model> {
        use crate::repositories::bars::{BarsQuery, find_bars};
        find_bars(
            db,
            BarsQuery {
                instrument_id: instrument_id.to_string(),
                timeframe: "1d".to_string(),
                from: None,
                to: None,
            },
        )
        .await
        .expect("find_bars failed")
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

    #[rstest]
    #[case::undetected_treats_today_as_the_upper_bound(
        None,
        NaiveDate::from_ymd_opt(2025, 6, 1).expect("date")
    )]
    #[case::detected_uses_the_detected_upper_bound(
        Some((
            NaiveDate::from_ymd_opt(2020, 4, 1).expect("date"),
            NaiveDate::from_ymd_opt(2022, 4, 1).expect("date"),
        )),
        NaiveDate::from_ymd_opt(2022, 4, 1).expect("date")
    )]
    fn latest_fetchable_date_cases(
        #[case] known_range: Option<(NaiveDate, NaiveDate)>,
        #[case] expected: NaiveDate,
    ) {
        let today = NaiveDate::from_ymd_opt(2025, 6, 1).expect("date");
        assert_eq!(latest_fetchable_date(today, known_range), expected);
    }

    #[sqlx::test(migrations = false)]
    async fn backfill_saves_bars_to_db(pool: PgPool) {
        let db = create_test_db(pool).await;
        insert_test_instrument(&db, "7203").await;

        // probe レンジ (20 年) 内に収まる適当な日付を使う
        let to = Utc::now().date_naive() - Duration::days(2);
        let bars = vec![
            make_bar("7203", to - Duration::days(2), 100),
            make_bar("7203", to - Duration::days(1), 105),
        ];

        let provider = MockProvider::new()
            .with_instruments(vec![sample_instrument("7203")])
            .with_bars(bars);

        backfill_daily_bars(&db, &provider, "7203").await;

        let result = find_all_bars(&db, "7203").await;

        assert_eq!(result.len(), 2);
    }

    #[sqlx::test(migrations = false)]
    async fn backfill_uses_known_fetchable_range_when_detected(pool: PgPool) {
        let db = create_test_db(pool).await;
        insert_test_instrument(&db, "7203").await;

        let known_from = NaiveDate::from_ymd_opt(2020, 4, 1).expect("date");
        let known_to = NaiveDate::from_ymd_opt(2022, 4, 1).expect("date");

        let in_range = make_bar("7203", known_from + Duration::days(1), 100);
        let out_of_range = make_bar("7203", known_to + Duration::days(1), 999);

        let provider = MockProvider::new()
            .with_instruments(vec![sample_instrument("7203")])
            .with_bars(vec![in_range, out_of_range])
            .with_known_fetchable_range(known_from, known_to);

        backfill_daily_bars(&db, &provider, "7203").await;

        let result = find_all_bars(&db, "7203").await;

        // 検出済み範囲外の bar はリクエストされないため保存されない
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].close, Decimal::new(100, 0));
    }

    #[sqlx::test(migrations = false)]
    async fn backfill_probes_far_history_when_range_is_undetected(pool: PgPool) {
        let db = create_test_db(pool).await;
        insert_test_instrument(&db, "7203").await;

        // Free プランの範囲 (2 年) よりずっと古い日付
        let old_date = Utc::now().date_naive() - Duration::days(365 * 15);
        let bar = make_bar("7203", old_date, 100);

        let provider = MockProvider::new()
            .with_instruments(vec![sample_instrument("7203")])
            .with_bars(vec![bar]);

        backfill_daily_bars(&db, &provider, "7203").await;

        let result = find_all_bars(&db, "7203").await;

        assert_eq!(result.len(), 1);
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
