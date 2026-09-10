//! 銘柄リストから直近終値を取得する。DB にバーが無い銘柄は DataProvider から
//! バックフィルしてから読み直す。

use std::collections::HashMap;

use chrono::NaiveDate;
use rust_decimal::Decimal;
use sea_orm::sea_query::OnConflict;
use sea_orm::{DatabaseConnection, EntityTrait, Set};

use crate::data_provider::DataProvider;
use crate::entities::instruments;
use crate::models::Timeframe;
use crate::repositories::bars::find_latest_bar;
use crate::services::backfill::backfill_daily_bars;

/// 銘柄ごとの直近終値と、取得できた中で最も新しい観測日。
#[derive(Debug, PartialEq)]
pub struct LatestPrices {
    pub prices: HashMap<String, Decimal>,
    /// 取得できた価格のうち最も新しい観測日。1 銘柄も価格を取得できなければ `None`。
    pub priced_at: Option<NaiveDate>,
}

/// `symbols` それぞれの最新終値を返す。バーが無い銘柄は DataProvider から
/// バックフィルしてから読み直す。取得元銘柄が存在しない等で結局バーが
/// 得られなかった銘柄は結果から省く (呼び出し元は該当銘柄の価格を null 扱いすること)。
///
/// ponytail: 既にバーがある銘柄は再取得しないため、直近の値動きより古いまま
/// 返ることがある。継続的な鮮度が要るなら sector_backfill.rs と同様の定期
/// ポーリングタスクを足す。
pub async fn fetch_latest_prices<P: DataProvider>(
    db: &DatabaseConnection,
    provider: Option<&P>,
    symbols: &[String],
) -> LatestPrices {
    let timeframe = Timeframe::Daily.to_string();
    let mut prices = HashMap::new();
    let mut priced_at: Option<NaiveDate> = None;

    for symbol in symbols {
        let mut bar = find_latest_bar(db, symbol, &timeframe)
            .await
            .unwrap_or_else(|e| {
                tracing::warn!(symbol, error = %e, "直近終値の取得に失敗");
                None
            });

        if bar.is_none()
            && let Some(provider) = provider
        {
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

        let Some(bar) = bar else { continue };
        let date = bar.timestamp.date_naive();
        priced_at = Some(priced_at.map_or(date, |current| current.max(date)));
        prices.insert(symbol.clone(), bar.close);
    }

    LatestPrices { prices, priced_at }
}

/// 価格取得の前提として `instruments` 行を保証する (`bars` の FK 制約のため)。
/// 既存の watchlist 由来の行があればそのまま使い、無ければ symbol を name として仮登録する。
async fn ensure_instrument_exists(
    db: &DatabaseConnection,
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
    use chrono::{Duration, NaiveDate, TimeZone, Utc};
    use rust_decimal::Decimal;
    use sqlx::PgPool;

    use super::*;
    use crate::models::instrument::{Instrument, Market};
    use crate::models::{Bar, Timeframe};
    use crate::repositories::bars::upsert_bars;
    use crate::testing::{MockProvider, create_test_db};

    fn sample_instrument(id: &str) -> Instrument {
        Instrument {
            id: id.to_string(),
            name: format!("Test {id}"),
            market: Market::Tse,
            sector: None,
        }
    }

    /// `backfill_daily_bars` が使う取得可能範囲内に収まる日付の bar を作る
    fn backfillable_bar(instrument_id: &str, close: i64) -> Bar {
        let today = Utc::now().date_naive();
        let date = today - Duration::weeks(12) - Duration::days(1);
        make_bar(instrument_id, date, close)
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

    #[sqlx::test(migrations = false)]
    async fn returns_existing_bar_without_calling_provider(pool: PgPool) {
        let db = create_test_db(pool).await;
        insert_test_instrument(&db, "7203").await;
        let date = NaiveDate::from_ymd_opt(2025, 1, 6).expect("date");
        upsert_bars(&db, vec![make_bar("7203", date, 100)])
            .await
            .expect("upsert");

        let provider = MockProvider::new();
        let result = fetch_latest_prices(&db, Some(&provider), &["7203".to_string()]).await;

        assert_eq!(
            result,
            LatestPrices {
                prices: HashMap::from([("7203".to_string(), Decimal::new(100, 0))]),
                priced_at: Some(date),
            }
        );
        assert!(provider.calls.lock().expect("lock").is_empty());
    }

    #[sqlx::test(migrations = false)]
    async fn backfills_missing_bar_and_creates_instrument(pool: PgPool) {
        let db = create_test_db(pool).await;
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
        assert_eq!(provider.calls.lock().expect("lock").as_slice(), ["7203"]);
    }

    #[sqlx::test(migrations = false)]
    async fn priced_at_is_the_max_date_across_symbols(pool: PgPool) {
        let db = create_test_db(pool).await;
        insert_test_instrument(&db, "7203").await;
        insert_test_instrument(&db, "6758").await;
        let older = NaiveDate::from_ymd_opt(2025, 1, 6).expect("date");
        let newer = NaiveDate::from_ymd_opt(2025, 1, 8).expect("date");
        upsert_bars(&db, vec![make_bar("7203", older, 100)])
            .await
            .expect("upsert");
        upsert_bars(&db, vec![make_bar("6758", newer, 300)])
            .await
            .expect("upsert");

        let provider = MockProvider::new();
        let result = fetch_latest_prices(
            &db,
            Some(&provider),
            &["7203".to_string(), "6758".to_string()],
        )
        .await;

        assert_eq!(
            result,
            LatestPrices {
                prices: HashMap::from([
                    ("7203".to_string(), Decimal::new(100, 0)),
                    ("6758".to_string(), Decimal::new(300, 0)),
                ]),
                priced_at: Some(newer),
            }
        );
    }

    #[sqlx::test(migrations = false)]
    async fn missing_bar_is_omitted_when_provider_is_none(pool: PgPool) {
        let db = create_test_db(pool).await;

        let result = fetch_latest_prices::<MockProvider>(&db, None, &["7203".to_string()]).await;

        assert_eq!(
            result,
            LatestPrices {
                prices: HashMap::new(),
                priced_at: None,
            }
        );
    }

    #[sqlx::test(migrations = false)]
    async fn provider_error_is_skipped_and_other_symbols_still_processed(pool: PgPool) {
        let db = create_test_db(pool).await;
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
