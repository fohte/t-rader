//! J-Quants の日足を日付指定で定期的に取り込む定期タスク。
//!
//! 対象は全上場銘柄 (`date` のみ指定して一括取得、銘柄の絞り込みは行わない)。取り込み
//! 済みの営業日は `jquants_daily_bars_ingested_date` に記録し、直近 400 営業日のうち
//! 未記録の日だけを取りに行く。直近 7 営業日は遡及訂正を拾うため記録の有無によらず
//! 毎回取り直す。引け後の公開時刻 (16:30 JST 前後) に依存しないよう、poll 間隔は 1
//! 時間にする。

use std::collections::HashSet;
use std::time::Duration;

use chrono::{Duration as ChronoDuration, NaiveDate, Utc};
use sea_orm::sea_query::OnConflict;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};
use tokio::task::JoinHandle;

use crate::data_provider::{MarketDailyBarSource, SharedMarketDailyBarSource};
use crate::date_utils::latest_business_day;
use crate::entities::{instruments, jquants_daily_bars_ingested_date};
use crate::error::AppError;
use crate::models::Bar;
use crate::repositories::bars::upsert_bars;

/// poll task のデフォルト実行間隔。
pub const DEFAULT_INTERVAL: Duration = Duration::from_secs(60 * 60);

/// 欠けが無いことを保証する対象の営業日数。
const TARGET_BUSINESS_DAYS: usize = 400;

/// 遡及訂正を拾うため、記録の有無によらず常に再取得する直近営業日数。
const REFETCH_WINDOW_BUSINESS_DAYS: usize = 7;

/// poll サイクルの結果統計
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct IngestStats {
    pub days_attempted: usize,
    pub bars_upserted: usize,
}

/// `to` を含めて過去に遡り、直近 `count` 営業日を古い順で返す。
fn recent_business_days(to: NaiveDate, count: usize) -> Vec<NaiveDate> {
    let mut days = Vec::with_capacity(count);
    let mut date = to;
    while days.len() < count {
        if latest_business_day(date) == date {
            days.push(date);
        }
        date -= ChronoDuration::days(1);
    }
    days.reverse();
    days
}

/// 取り込み対象日を判定する。`business_days` は古い順で、直近
/// `REFETCH_WINDOW_BUSINESS_DAYS` 日は `ingested` に関わらず常に対象、それより前は
/// `ingested` に無い日だけを対象にする。
fn target_dates(business_days: &[NaiveDate], ingested: &HashSet<NaiveDate>) -> Vec<NaiveDate> {
    let refetch_from_index = business_days
        .len()
        .saturating_sub(REFETCH_WINDOW_BUSINESS_DAYS);
    business_days
        .iter()
        .enumerate()
        .filter(|(i, date)| *i >= refetch_from_index || !ingested.contains(date))
        .map(|(_, date)| *date)
        .collect()
}

/// `from` 以降で記録済みの取り込み日を返す。
async fn find_ingested_dates(
    db: &DatabaseConnection,
    from: NaiveDate,
) -> Result<HashSet<NaiveDate>, AppError> {
    let rows = jquants_daily_bars_ingested_date::Entity::find()
        .filter(jquants_daily_bars_ingested_date::Column::Date.gte(from))
        .all(db)
        .await?;
    Ok(rows.into_iter().map(|row| row.date).collect())
}

/// `bars` の FK 制約のため `instruments` 行を保証する。銘柄名などの詳細は把握できないため、
/// 銘柄コードをそのまま仮の name として登録する。
async fn ensure_instruments_exist(
    db: &DatabaseConnection,
    instrument_ids: &HashSet<String>,
) -> Result<(), AppError> {
    if instrument_ids.is_empty() {
        return Ok(());
    }

    let models = instrument_ids.iter().map(|id| instruments::ActiveModel {
        id: Set(id.clone()),
        name: Set(id.clone()),
        market: Set("TSE".to_string()),
        sector: Set(None),
    });

    instruments::Entity::insert_many(models)
        .on_conflict(
            OnConflict::column(instruments::Column::Id)
                .do_nothing()
                .to_owned(),
        )
        .exec_without_returning(db)
        .await?;

    Ok(())
}

/// `date` の取り込み日を記録する。既に記録済みなら何もしない。
async fn mark_ingested(db: &DatabaseConnection, date: NaiveDate) -> Result<(), AppError> {
    jquants_daily_bars_ingested_date::Entity::insert(
        jquants_daily_bars_ingested_date::ActiveModel { date: Set(date) },
    )
    .on_conflict(
        OnConflict::column(jquants_daily_bars_ingested_date::Column::Date)
            .do_nothing()
            .to_owned(),
    )
    .exec_without_returning(db)
    .await?;
    Ok(())
}

/// `date` の日足を取り込む。まだ公開されていない (0 件) 場合は取り込み日を記録せず
/// `0` を返す。
async fn ingest_date(
    db: &DatabaseConnection,
    source: &dyn MarketDailyBarSource,
    date: NaiveDate,
) -> Result<usize, AppError> {
    let bars: Vec<Bar> = source.fetch_daily_bars_by_date(date).await?;

    if bars.is_empty() {
        return Ok(0);
    }

    let instrument_ids: HashSet<String> = bars.iter().map(|b| b.instrument_id.clone()).collect();
    ensure_instruments_exist(db, &instrument_ids).await?;

    let bar_count = bars.len();
    upsert_bars(db, bars).await?;
    mark_ingested(db, date).await?;

    Ok(bar_count)
}

/// 日足を取り込む 1 サイクル。取得元が取得できる範囲を返さない間は取り込まない。
pub async fn run_ingest_cycle(
    db: &DatabaseConnection,
    source: &dyn MarketDailyBarSource,
) -> Result<IngestStats, AppError> {
    let today = Utc::now().date_naive();
    let Some(range) = source.fetchable_range(today) else {
        tracing::debug!("日足を取得できないため取り込みをスキップします");
        return Ok(IngestStats::default());
    };

    // 配信遅延がある場合、素の today だとまだ提供されていない日を対象にして 400 エラーを
    // 繰り返してしまうため、取得できる範囲でクランプする。
    let to = latest_business_day(range.to.min(today));
    let business_days = recent_business_days(to, TARGET_BUSINESS_DAYS);
    let Some(&earliest) = business_days.first() else {
        return Ok(IngestStats::default());
    };

    let ingested = find_ingested_dates(db, earliest).await?;
    let targets = target_dates(&business_days, &ingested);

    let mut stats = IngestStats::default();
    for date in targets {
        stats.days_attempted += 1;
        match ingest_date(db, source, date).await {
            Ok(0) => {
                tracing::debug!(%date, "この日の日足はまだ公開されていません");
            }
            Ok(n) => stats.bars_upserted += n,
            Err(e) => {
                tracing::warn!(%date, error = %e, "日足の取り込みに失敗、この日をスキップします");
            }
        }
    }

    Ok(stats)
}

/// poll task を起動する。1 回目は即実行し、その後 `interval` で繰り返す。
pub fn spawn_poll(
    db: DatabaseConnection,
    source: SharedMarketDailyBarSource,
    interval: Duration,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(interval);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            ticker.tick().await;
            match run_ingest_cycle(&db, source.as_ref()).await {
                Ok(stats) => {
                    tracing::debug!(
                        days_attempted = stats.days_attempted,
                        bars_upserted = stats.bars_upserted,
                        "daily bars ingest cycle completed",
                    );
                }
                Err(err) => {
                    tracing::warn!(%err, "daily bars ingest cycle failed");
                }
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use rstest::rstest;
    use sea_orm::{ActiveModelTrait, EntityTrait};
    use sqlx::PgPool;

    use super::*;
    use crate::data_provider::jquants::JQuantsClient;
    use crate::data_provider::jquants::mock::{JQuantsMockServer, MockBar};
    use crate::models::jquants_plan::JQuantsPlan;
    use crate::repositories::bars::{BarsQuery, find_bars};
    use crate::testing::create_test_db;

    fn date(year: i32, month: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(year, month, day).expect("valid date")
    }

    fn mock_bar(date_str: &str, code: &str, close: f64) -> MockBar {
        MockBar {
            date: date_str.to_string(),
            code: code.to_string(),
            adj_open: Some(close),
            adj_high: Some(close + 10.0),
            adj_low: Some(close - 10.0),
            adj_close: Some(close),
            adj_volume: Some(1000.0),
        }
    }

    async fn seed_ingested(db: &DatabaseConnection, date: NaiveDate) {
        jquants_daily_bars_ingested_date::ActiveModel { date: Set(date) }
            .insert(db)
            .await
            .expect("seed ingested date");
    }

    #[test]
    fn test_recent_business_days_returns_oldest_first_skipping_weekends() {
        // 2025-06-10 (火) を起点に直近 3 営業日 → 6/6 (金), 6/9 (月), 6/10 (火)
        // (6/7, 6/8 は土日)
        let days = recent_business_days(date(2025, 6, 10), 3);

        assert_eq!(
            days,
            vec![date(2025, 6, 6), date(2025, 6, 9), date(2025, 6, 10)]
        );
    }

    #[rstest]
    #[case::empty_ingested_targets_everything(
        vec![date(2025, 1, 2), date(2025, 1, 3), date(2025, 1, 6)],
        vec![],
        vec![date(2025, 1, 2), date(2025, 1, 3), date(2025, 1, 6)]
    )]
    // 9 営業日のうち先頭 1 日 (直近 7 営業日の再取得ウィンドウ外) だけを記録済みにする →
    // その 1 日だけが対象から外れ、残り 8 日 (ウィンドウ内の 7 日 + 未記録の 1 日) が対象になる
    #[case::ingested_day_outside_refetch_window_is_skipped(
        vec![
            date(2025, 1, 1), date(2025, 1, 2), date(2025, 1, 3), date(2025, 1, 6),
            date(2025, 1, 7), date(2025, 1, 8), date(2025, 1, 9), date(2025, 1, 10),
            date(2025, 1, 13),
        ],
        vec![date(2025, 1, 1)],
        vec![
            date(2025, 1, 2), date(2025, 1, 3), date(2025, 1, 6), date(2025, 1, 7),
            date(2025, 1, 8), date(2025, 1, 9), date(2025, 1, 10), date(2025, 1, 13),
        ]
    )]
    fn test_target_dates(
        #[case] business_days: Vec<NaiveDate>,
        #[case] ingested: Vec<NaiveDate>,
        #[case] expected: Vec<NaiveDate>,
    ) {
        let ingested: HashSet<NaiveDate> = ingested.into_iter().collect();
        assert_eq!(target_dates(&business_days, &ingested), expected);
    }

    #[test]
    fn test_target_dates_always_refetches_within_the_lookback_window_even_if_ingested() {
        // REFETCH_WINDOW_BUSINESS_DAYS (7) 件ちょうどの営業日、全て記録済みでも
        // 全日が再取得対象になる
        let business_days: Vec<NaiveDate> = (2..=8)
            .map(|day| date(2025, 1, day))
            .filter(|d| latest_business_day(*d) == *d)
            .collect();
        let ingested: HashSet<NaiveDate> = business_days.iter().copied().collect();

        assert_eq!(target_dates(&business_days, &ingested), business_days);
    }

    #[sqlx::test(migrations = false)]
    async fn test_skips_when_plan_is_unset(pool: PgPool) {
        let db = create_test_db(pool).await;
        let client = JQuantsClient::new("test-api-key".to_string()).expect("client");

        let stats = run_ingest_cycle(&db, &client).await.expect("cycle ok");

        assert_eq!(stats, IngestStats::default());
    }

    #[sqlx::test(migrations = false)]
    async fn test_ingests_bars_and_creates_missing_instruments(pool: PgPool) {
        let db = create_test_db(pool).await;
        let mock = JQuantsMockServer::start().await;
        let client = mock.client().expect("client");
        client.set_manual_plan(Some(JQuantsPlan::Standard));

        let today = Utc::now().date_naive();
        let to = latest_business_day(today);

        // 直近 400 営業日のうち、`to` を除く全日を既に記録済みにしておき、対象を未記録の `to` を
        // 含む直近 7 営業日に絞る
        let business_days = recent_business_days(to, TARGET_BUSINESS_DAYS);
        for &d in &business_days[..business_days.len() - 1] {
            seed_ingested(&db, d).await;
        }

        let to_str = to.format("%Y-%m-%d").to_string();
        mock.daily_bars_by_date()
            .date(&to_str)
            .bars(vec![mock_bar(&to_str, "72030", 100.0)])
            .ok()
            .await;

        let stats = run_ingest_cycle(&db, &client).await.expect("cycle ok");

        // 直近 REFETCH_WINDOW_BUSINESS_DAYS (7) 日は記録済みでも常に再取得対象になるため、
        // 実際に取り込みを試みるのは `to` の 1 日だけではなく 7 日分になる。`to` 以外の
        // 6 日は mock 未設定 (404) で失敗し、bars_upserted には加算されない。
        assert_eq!(
            stats,
            IngestStats {
                days_attempted: 7,
                bars_upserted: 1,
            }
        );

        let bars = find_bars(
            &db,
            BarsQuery {
                instrument_id: "7203".to_string(),
                timeframe: "1d".to_string(),
                from: None,
                to: None,
            },
        )
        .await
        .expect("find_bars ok");
        assert_eq!(bars.len(), 1);
        assert_eq!(bars[0].close, rust_decimal::Decimal::new(100, 0));

        let instrument = instruments::Entity::find_by_id("7203".to_string())
            .one(&db)
            .await
            .expect("query ok")
            .expect("instrument row exists");
        assert_eq!(instrument.name, "7203");

        let ingested = find_ingested_dates(&db, to).await.expect("query ok");
        assert_eq!(ingested, HashSet::from([to]));
    }

    #[sqlx::test(migrations = false)]
    async fn test_does_not_mark_unpublished_day_as_ingested(pool: PgPool) {
        let db = create_test_db(pool).await;
        let mock = JQuantsMockServer::start().await;
        let client = mock.client().expect("client");
        client.set_manual_plan(Some(JQuantsPlan::Standard));

        let today = Utc::now().date_naive();
        let to = latest_business_day(today);
        let business_days = recent_business_days(to, TARGET_BUSINESS_DAYS);
        for &d in &business_days[..business_days.len() - 1] {
            seed_ingested(&db, d).await;
        }

        mock.daily_bars_by_date()
            .date(&to.format("%Y-%m-%d").to_string())
            .bars(vec![])
            .ok()
            .await;

        let stats = run_ingest_cycle(&db, &client).await.expect("cycle ok");

        // 直近 7 営業日は常に再取得対象になる。`to` 以外の 6 日は mock 未設定 (404) で失敗する
        assert_eq!(
            stats,
            IngestStats {
                days_attempted: 7,
                bars_upserted: 0,
            }
        );
        let ingested = find_ingested_dates(&db, to).await.expect("query ok");
        assert_eq!(ingested, HashSet::new());
    }

    #[sqlx::test(migrations = false)]
    async fn test_continues_past_days_that_fail_to_fetch(pool: PgPool) {
        let db = create_test_db(pool).await;
        let mock = JQuantsMockServer::start().await;
        let client = mock.client().expect("client");
        client.set_manual_plan(Some(JQuantsPlan::Standard));

        let today = Utc::now().date_naive();
        let to = latest_business_day(today);
        let business_days = recent_business_days(to, TARGET_BUSINESS_DAYS);
        let prev = business_days[business_days.len() - 2];
        for &d in &business_days[..business_days.len() - 2] {
            seed_ingested(&db, d).await;
        }

        // `prev` 以外の日は mock を用意しない (マッチせず 404 → fetch エラー) が、
        // サイクル全体は失敗させず、直近 7 営業日すべてを attempted として扱う
        let prev_str = prev.format("%Y-%m-%d").to_string();
        mock.daily_bars_by_date()
            .date(&prev_str)
            .bars(vec![mock_bar(&prev_str, "72030", 100.0)])
            .ok()
            .await;

        let stats = run_ingest_cycle(&db, &client).await.expect("cycle ok");

        assert_eq!(
            stats,
            IngestStats {
                days_attempted: 7,
                bars_upserted: 1,
            }
        );
    }
}
