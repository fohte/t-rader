use chrono::{Duration, NaiveDate, Utc};
use sea_orm::DatabaseConnection;

use crate::data_provider::{DataProvider, DateRange};
use crate::models::Timeframe;
use crate::repositories::bars::upsert_bars;

/// 契約範囲が未学習のときに試す確認用の範囲 (Premium 相当の最大範囲)。この範囲で
/// リクエストし、実際の契約範囲を成功レスポンスまたは 400 エラーメッセージから学習する。
/// 学習後はこの範囲を使わず、学習済みの範囲 (`known_range`) を使い回す。
const PROBE_MAX_HISTORY_DAYS: i64 = 365 * 20;

/// `backfill_daily_bars` が実際に取得できる最新日。`known_range` が学習済みならその
/// 上限日を返す。未学習の間は「今日より古いデータは全て取得を試みるべき」として
/// `today` を返す (= 既存バーは常に stale 扱いになり、`fetch_latest_prices` が
/// backfill を試みて学習のきっかけを作る)。契約範囲外だった Free プラン時代の上限
/// (12 週前) に黙って留まり続けることは絶対にしない。
/// 保有時価の評価上限 (`market_price::fetch_latest_prices`) もこの関数を経由して
/// 同じ値を参照するため、取得側と評価側で上限がずれることはない。
pub(crate) fn latest_fetchable_date(
    today: NaiveDate,
    known_range: Option<(NaiveDate, NaiveDate)>,
) -> NaiveDate {
    known_range.map_or_else(
        || {
            tracing::warn!("契約範囲が未学習のため、上限を today として扱い取得を試みます");
            today
        },
        |(_, to)| to,
    )
}

/// 指定銘柄の日足データをバックフィルする。
///
/// `data_provider.known_fetchable_range()` が契約範囲を学習済みならその範囲を、
/// 未学習ならまず最大範囲 (Premium 相当) で問い合わせて実際の契約範囲を学習する
/// (学習は `DataProvider` 実装側の責務。例えば J-Quants は 400 エラーメッセージから
/// 学習し、以降のリクエストで使い回す)。
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

    let bars = match data_provider.fetch_daily_bars(instrument_id, &range).await {
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
    #[case::unlearned_treats_today_as_the_upper_bound(
        None,
        NaiveDate::from_ymd_opt(2025, 6, 1).expect("date")
    )]
    #[case::learned_uses_the_learned_upper_bound(
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
    async fn backfill_uses_known_fetchable_range_when_learned(pool: PgPool) {
        let db = create_test_db(pool).await;
        insert_test_instrument(&db, "7203").await;

        let known_from = NaiveDate::from_ymd_opt(2020, 4, 1).expect("date");
        let known_to = NaiveDate::from_ymd_opt(2022, 4, 1).expect("date");

        // 学習済み範囲内の bar と範囲外の bar を両方用意する
        let in_range = make_bar("7203", known_from + Duration::days(1), 100);
        let out_of_range = make_bar("7203", known_to + Duration::days(1), 999);

        let provider = MockProvider::new()
            .with_instruments(vec![sample_instrument("7203")])
            .with_bars(vec![in_range, out_of_range])
            .with_known_fetchable_range(known_from, known_to);

        backfill_daily_bars(&db, &provider, "7203").await;

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

        // 学習済み範囲外の bar はリクエストされないため保存されない
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].close, Decimal::new(100, 0));
    }

    #[sqlx::test(migrations = false)]
    async fn backfill_probes_far_history_when_range_is_unlearned(pool: PgPool) {
        let db = create_test_db(pool).await;
        insert_test_instrument(&db, "7203").await;

        // Free プランの範囲 (2 年) よりずっと古い日付
        let old_date = Utc::now().date_naive() - Duration::days(365 * 15);
        let bar = make_bar("7203", old_date, 100);

        let provider = MockProvider::new()
            .with_instruments(vec![sample_instrument("7203")])
            .with_bars(vec![bar]);

        backfill_daily_bars(&db, &provider, "7203").await;

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
