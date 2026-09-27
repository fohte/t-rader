//! 銘柄リストから直近終値を取得する。バーが `backfill_daily_bars` の取得可能上限に
//! 届いていない銘柄は日足データ取得元から再取得してから読み直す。

use std::collections::HashMap;

use chrono::{NaiveDate, Utc};
use rust_decimal::Decimal;
use sea_orm::sea_query::OnConflict;
use sea_orm::{EntityTrait, Set};

use crate::data_provider::DailyBarSource;
use crate::date_utils::latest_business_day;
use crate::entities::instruments;
use crate::models::Timeframe;
use crate::repositories::bars::find_latest_bar;
use crate::services::backfill::{backfill_daily_bars, latest_fetchable_date};

/// 銘柄ごとの直近終値と、その観測日。
#[derive(Debug, PartialEq)]
pub struct LatestPrices {
    /// `priced_at` と同じ観測日の終値のみを含む (異なる日付の終値が混在することはない)。
    pub prices: HashMap<String, Decimal>,
    /// `prices` の全銘柄に共通する観測日。1 銘柄も価格を取得できなければ `None`。
    pub priced_at: Option<NaiveDate>,
}

/// `symbols` それぞれの最新終値を返す。取得可能上限日に届いていない銘柄は
/// 日足データ取得元から再取得を試みる。全銘柄中の最新観測日 (`priced_at`) に満たない
/// 銘柄は結果から省かれる。
pub async fn fetch_latest_prices(
    db: &impl sea_orm::ConnectionTrait,
    provider: Option<&dyn DailyBarSource>,
    symbols: &[String],
) -> LatestPrices {
    let timeframe = Timeframe::Daily.to_string();
    let known_range = provider.and_then(|p| p.known_fetchable_range());
    let fetchable_ceiling =
        latest_business_day(latest_fetchable_date(Utc::now().date_naive(), known_range));
    let mut bars: HashMap<String, (NaiveDate, Decimal)> = HashMap::new();

    for symbol in symbols {
        let mut bar = find_latest_bar(db, symbol, &timeframe)
            .await
            .unwrap_or_else(|e| {
                tracing::warn!(symbol, error = %e, "直近終値の取得に失敗");
                None
            });

        let is_stale = bar
            .as_ref()
            .is_none_or(|b| b.timestamp.date_naive() < fetchable_ceiling);
        if is_stale && let Some(provider) = provider {
            match ensure_instrument_exists(db, symbol).await {
                Ok(()) => {
                    backfill_daily_bars(db, provider, symbol).await;
                    bar = find_latest_bar(db, symbol, &timeframe)
                        .await
                        .unwrap_or_else(|e| {
                            tracing::warn!(symbol, error = %e, "バックフィル後の直近終値取得に失敗");
                            None
                        });
                }
                Err(e) => {
                    tracing::warn!(symbol, error = %e, "instruments 行の作成に失敗");
                }
            }
        }

        if let Some(bar) = bar {
            bars.insert(symbol.clone(), (bar.timestamp.date_naive(), bar.close));
        }
    }

    let (prices, priced_at) = select_common_priced_at(bars);
    LatestPrices { prices, priced_at }
}

/// 取得できたバーの集合から、全銘柄に共通する最新観測日の終値だけを残す。
fn select_common_priced_at(
    bars: HashMap<String, (NaiveDate, Decimal)>,
) -> (HashMap<String, Decimal>, Option<NaiveDate>) {
    let priced_at = bars.values().map(|(date, _)| *date).max();
    let prices = priced_at.map_or_else(HashMap::new, |latest| {
        bars.into_iter()
            .filter(|(symbol, (date, _))| {
                if *date == latest {
                    return true;
                }
                tracing::warn!(symbol, %date, %latest, "観測日が最新銘柄と食い違うため価格から除外");
                false
            })
            .map(|(symbol, (_, close))| (symbol, close))
            .collect()
    });
    (prices, priced_at)
}

/// 価格取得の前提として `instruments` 行を保証する (`bars` の FK 制約のため)。
/// 銘柄情報が未登録なら symbol を name として仮登録する。
async fn ensure_instrument_exists(
    db: &impl sea_orm::ConnectionTrait,
    symbol: &str,
) -> Result<(), sea_orm::DbErr> {
    let model = instruments::ActiveModel {
        id: Set(symbol.to_string()),
        name: Set(symbol.to_string()),
        market: Set("TSE".to_string()),
        sector: Set(None),
    };
    instruments::Entity::insert(model)
        .on_conflict(
            OnConflict::column(instruments::Column::Id)
                .do_nothing()
                .to_owned(),
        )
        .exec_without_returning(db)
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::instrument::{Instrument, Market};
    use crate::models::{Bar, Timeframe};
    use crate::repositories::bars::upsert_bars;
    use crate::services::backfill::latest_fetchable_date;
    use crate::testing::MockProvider;
    use chrono::{Duration, NaiveDate, TimeZone, Utc};
    use rust_decimal::Decimal;

    fn sample_instrument(id: &str) -> Instrument {
        Instrument {
            id: id.to_string(),
            name: format!("Test {id}"),
            market: Market::Tse,
            sector: None,
            product_category: None,
        }
    }

    /// `backfill_daily_bars` が使う取得可能範囲内に収まる日付の bar を作る
    fn backfillable_bar(instrument_id: &str, close: i64) -> Bar {
        let today = Utc::now().date_naive();
        let date = today - Duration::weeks(12) - Duration::days(1);
        make_bar(instrument_id, date, close)
    }

    /// fetch_latest_prices が「これ以上新しくならない」と判定する境界日ちょうどの bar を作る
    /// MockProvider が取得範囲を公開しない場合は today を上限として判定する。
    fn ceiling_bar(instrument_id: &str, close: i64) -> Bar {
        let ceiling = latest_business_day(latest_fetchable_date(Utc::now().date_naive(), None));
        make_bar(instrument_id, ceiling, close)
    }

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

    fn assert_provider_calls(provider: &MockProvider, expected: &[&str]) {
        assert_eq!(provider.calls.lock().expect("lock").as_slice(), expected);
    }

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

    #[test]
    fn select_common_priced_at_excludes_symbols_with_mismatched_dates() {
        let older = NaiveDate::from_ymd_opt(2025, 1, 6).expect("date");
        let newer = NaiveDate::from_ymd_opt(2025, 1, 8).expect("date");
        let bars = HashMap::from([
            ("7203".to_string(), (older, Decimal::new(100, 0))),
            ("6758".to_string(), (newer, Decimal::new(300, 0))),
        ]);

        let result = select_common_priced_at(bars);

        assert_eq!(
            result,
            (
                HashMap::from([("6758".to_string(), Decimal::new(300, 0))]),
                Some(newer),
            )
        );
    }

    #[backend_test_macros::database_test]
    async fn skips_provider_when_bar_already_reaches_the_fetchable_ceiling(
        db: crate::database::DatabaseHandle,
    ) {
        insert_test_instrument(&db, "7203").await;
        let bar = ceiling_bar("7203", 100);
        let date = bar.timestamp.date_naive();
        upsert_bars(&db, vec![bar]).await.expect("upsert");

        let provider = MockProvider::new();
        let result = fetch_latest_prices(&db, Some(&provider), &["7203".to_string()]).await;

        assert_eq!(
            result,
            LatestPrices {
                prices: HashMap::from([("7203".to_string(), Decimal::new(100, 0))]),
                priced_at: Some(date),
            }
        );
        assert_provider_calls(&provider, &[]);
    }

    #[backend_test_macros::database_test]
    async fn stops_calling_provider_once_bar_reaches_the_fetchable_ceiling(
        db: crate::database::DatabaseHandle,
    ) {
        let provider = MockProvider::new()
            .with_instruments(vec![sample_instrument("7203")])
            .with_bars(vec![ceiling_bar("7203", 200)]);

        fetch_latest_prices(&db, Some(&provider), &["7203".to_string()]).await;
        assert_provider_calls(&provider, &["7203"]);

        fetch_latest_prices(&db, Some(&provider), &["7203".to_string()]).await;
        assert_provider_calls(&provider, &["7203"]);
    }

    #[backend_test_macros::database_test]
    async fn refetches_stale_bar_even_when_one_already_exists(db: crate::database::DatabaseHandle) {
        insert_test_instrument(&db, "7203").await;
        let stale_date = NaiveDate::from_ymd_opt(2025, 1, 6).expect("date");
        upsert_bars(&db, vec![make_bar("7203", stale_date, 100)])
            .await
            .expect("upsert");

        let fresh_bar = backfillable_bar("7203", 200);
        let expected_date = fresh_bar.timestamp.date_naive();
        let provider = MockProvider::new()
            .with_instruments(vec![sample_instrument("7203")])
            .with_bars(vec![fresh_bar]);

        let result = fetch_latest_prices(&db, Some(&provider), &["7203".to_string()]).await;

        assert_eq!(
            result,
            LatestPrices {
                prices: HashMap::from([("7203".to_string(), Decimal::new(200, 0))]),
                priced_at: Some(expected_date),
            }
        );
        assert_provider_calls(&provider, &["7203"]);
    }

    #[backend_test_macros::database_test]
    async fn backfills_missing_bar_and_creates_instrument(db: crate::database::DatabaseHandle) {
        let bar = backfillable_bar("7203", 200);
        let expected_date = bar.timestamp.date_naive();
        let provider = MockProvider::new()
            .with_instruments(vec![sample_instrument("7203")])
            .with_bars(vec![bar]);

        let result = fetch_latest_prices(&db, Some(&provider), &["7203".to_string()]).await;

        assert_eq!(
            result,
            LatestPrices {
                prices: HashMap::from([("7203".to_string(), Decimal::new(200, 0))]),
                priced_at: Some(expected_date),
            }
        );
        assert_provider_calls(&provider, &["7203"]);
    }

    #[backend_test_macros::database_test]
    async fn stale_symbols_that_cannot_catch_up_are_excluded_from_prices(
        db: crate::database::DatabaseHandle,
    ) {
        insert_test_instrument(&db, "7203").await;
        insert_test_instrument(&db, "6758").await;
        let stale_date = NaiveDate::from_ymd_opt(2025, 1, 6).expect("date");
        upsert_bars(&db, vec![make_bar("7203", stale_date, 100)])
            .await
            .expect("upsert");
        upsert_bars(&db, vec![make_bar("6758", stale_date, 300)])
            .await
            .expect("upsert");

        let fresh_bar = backfillable_bar("6758", 350);
        let fresh_date = fresh_bar.timestamp.date_naive();
        let provider = MockProvider::new()
            .with_instruments(vec![sample_instrument("6758")])
            .with_bars(vec![fresh_bar]);

        let result = fetch_latest_prices(
            &db,
            Some(&provider),
            &["7203".to_string(), "6758".to_string()],
        )
        .await;

        assert_eq!(
            result,
            LatestPrices {
                prices: HashMap::from([("6758".to_string(), Decimal::new(350, 0))]),
                priced_at: Some(fresh_date),
            }
        );
    }

    #[backend_test_macros::database_test]
    async fn missing_bar_is_omitted_when_provider_is_none(db: crate::database::DatabaseHandle) {
        let result = fetch_latest_prices(&db, None, &["7203".to_string()]).await;

        assert_eq!(
            result,
            LatestPrices {
                prices: HashMap::new(),
                priced_at: None,
            }
        );
    }

    #[backend_test_macros::database_test]
    async fn provider_error_is_skipped_and_other_symbols_still_processed(
        db: crate::database::DatabaseHandle,
    ) {
        insert_test_instrument(&db, "6758").await;
        let date = NaiveDate::from_ymd_opt(2025, 1, 6).expect("date");
        upsert_bars(&db, vec![make_bar("6758", date, 300)])
            .await
            .expect("upsert");

        // "9999" は provider に存在しないため NotFound になる
        let provider = MockProvider::new();
        let result = fetch_latest_prices(
            &db,
            Some(&provider),
            &["9999".to_string(), "6758".to_string()],
        )
        .await;

        assert_eq!(
            result,
            LatestPrices {
                prices: HashMap::from([("6758".to_string(), Decimal::new(300, 0))]),
                priced_at: Some(date),
            }
        );
    }
}
