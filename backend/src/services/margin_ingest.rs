//! 信用取引週末残高・日々公表信用取引残高を日次で取得し、DB に蓄積する定期タスク。
//!
//! 取得元が取得できる範囲を返さない間は取り込まない。

use std::future::Future;
use std::time::Duration;

use chrono::{Duration as ChronoDuration, NaiveDate, Utc};
use sea_orm::DatabaseConnection;
use tokio::task::JoinHandle;

use crate::data_provider::{MarginSource, MarginSourceError, SharedMarginSource};
use crate::error::AppError;
use crate::repositories::{margin_alert, margin_interest};

/// poll task のデフォルト実行間隔 (1 日)。日次更新の J-Quants データに対して十分な頻度。
pub const DEFAULT_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);

/// 信用取引週末残高の提供開始日 (https://jpx-jquants.com/ja/spec/mkt-margin-int)
fn margin_interest_start_date() -> NaiveDate {
    NaiveDate::from_ymd_opt(2012, 2, 10).unwrap_or_default()
}

/// 日々公表信用取引残高の提供開始日 (https://jpx-jquants.com/ja/spec/mkt-margin-alert)
fn margin_alert_start_date() -> NaiveDate {
    NaiveDate::from_ymd_opt(2008, 5, 8).unwrap_or_default()
}

/// 専用のチェックポイントテーブルは持たず、テーブル内の最新日付から再開する。
/// margin_interest の訂正は上書きで反映されるため、この日数分さかのぼって再取得する。
/// margin_alert の訂正は PubDate が新しい行として追加されるため、この lookback では
/// 拾われない (新しい PubDate 側は次サイクル以降の forward 差分で取得される)。
const REFETCH_LOOKBACK_DAYS: i64 = 30;

/// 取得開始日を決定する。テーブルが空なら `earliest` から、既にデータがあれば
/// 最新日付から `REFETCH_LOOKBACK_DAYS` 日さかのぼった日 (ただし `earliest` 未満にはしない) から。
fn resolve_start_date(latest_stored: Option<NaiveDate>, earliest: NaiveDate) -> NaiveDate {
    match latest_stored {
        None => earliest,
        Some(latest) => (latest - ChronoDuration::days(REFETCH_LOOKBACK_DAYS)).max(earliest),
    }
}

/// 1 サイクルの取り込み結果統計
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct IngestStats {
    pub days_fetched: usize,
    pub rows_upserted: usize,
}

/// `start` から `end` (両端含む) まで日付を 1 日ずつ進め、`fetch`/`upsert` で取得・保存する。
/// 1 日分の取得・保存に失敗しても残りの日付は続行する。
async fn ingest_daily<'c, C, T, F, FetchFut, G, UpsertFut>(
    db: &'c C,
    source: &'c dyn MarginSource,
    start: NaiveDate,
    end: NaiveDate,
    label: &str,
    fetch: F,
    upsert: G,
) -> IngestStats
where
    C: sea_orm::ConnectionTrait,
    F: Fn(&'c dyn MarginSource, NaiveDate) -> FetchFut,
    FetchFut: Future<Output = Result<Vec<T>, MarginSourceError>>,
    G: Fn(&'c C, Vec<T>) -> UpsertFut,
    UpsertFut: Future<Output = Result<(), AppError>>,
{
    let mut stats = IngestStats::default();
    let mut date = start;
    while date <= end {
        match fetch(source, date).await {
            Ok(records) => {
                let count = records.len();
                match upsert(db, records).await {
                    Ok(()) => {
                        stats.days_fetched += 1;
                        stats.rows_upserted += count;
                    }
                    Err(e) => {
                        tracing::warn!(%date, %label, error = %e, "保存に失敗、次の日に進みます");
                    }
                }
            }
            Err(e) => {
                tracing::warn!(%date, %label, error = %e, "取得に失敗、次の日に進みます");
            }
        }
        date += ChronoDuration::days(1);
    }
    stats
}

/// 1 サイクル実行: margin_interest, margin_alert それぞれ未取得区間を取得・保存する。
/// 取得元が取得できる範囲を返さないなら何もしない。
pub async fn run_ingest_cycle(
    db: &impl sea_orm::ConnectionTrait,
    source: &dyn MarginSource,
    today: NaiveDate,
) -> Result<(IngestStats, IngestStats), AppError> {
    // 配信遅延を反映した上限日 (range.to) を超えては取得できない (backfill.rs::latest_fetchable_date と同様)
    let Some(range) = source.fetchable_range(today) else {
        tracing::debug!("信用残データを取得できないため、取り込みをスキップします");
        return Ok((IngestStats::default(), IngestStats::default()));
    };

    let interest_earliest = range.from.max(margin_interest_start_date());
    let interest_latest = margin_interest::find_latest_margin_interest_date(db).await?;
    let interest_start = resolve_start_date(interest_latest, interest_earliest);
    let interest_stats = ingest_daily(
        db,
        source,
        interest_start,
        range.to,
        "信用取引週末残高",
        MarginSource::fetch_margin_interest,
        margin_interest::upsert_margin_interest,
    )
    .await;

    let alert_earliest = range.from.max(margin_alert_start_date());
    let alert_latest = margin_alert::find_latest_margin_alert_pub_date(db).await?;
    let alert_start = resolve_start_date(alert_latest, alert_earliest);
    let alert_stats = ingest_daily(
        db,
        source,
        alert_start,
        range.to,
        "日々公表信用取引残高",
        MarginSource::fetch_margin_alert,
        margin_alert::upsert_margin_alert,
    )
    .await;

    Ok((interest_stats, alert_stats))
}

/// poll task を起動する。1 回目は即実行し、その後 `interval` で繰り返す。
pub fn spawn_poll(
    db: DatabaseConnection,
    source: SharedMarginSource,
    interval: Duration,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(interval);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            ticker.tick().await;
            let today = Utc::now().date_naive();
            match run_ingest_cycle(&db, source.as_ref(), today).await {
                Ok((interest_stats, alert_stats)) => {
                    tracing::info!(
                        ?interest_stats,
                        ?alert_stats,
                        "margin ingest cycle completed"
                    );
                }
                Err(e) => {
                    tracing::warn!(%e, "margin ingest cycle failed");
                }
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_provider::jquants::mock::{JQuantsMockServer, MockMarginInterestRow};
    use crate::models::jquants_plan::JQuantsPlan;
    use rstest::rstest;
    #[rstest]
    #[case::empty_table_uses_earliest(
        None,
        NaiveDate::from_ymd_opt(2020, 1, 1).expect("date"),
        NaiveDate::from_ymd_opt(2020, 1, 1).expect("date")
    )]
    #[case::existing_data_looks_back_from_latest(
        Some(NaiveDate::from_ymd_opt(2024, 6, 15).expect("date")),
        NaiveDate::from_ymd_opt(2020, 1, 1).expect("date"),
        NaiveDate::from_ymd_opt(2024, 5, 16).expect("date")
    )]
    #[case::lookback_does_not_go_before_earliest(
        Some(NaiveDate::from_ymd_opt(2020, 1, 10).expect("date")),
        NaiveDate::from_ymd_opt(2020, 1, 1).expect("date"),
        NaiveDate::from_ymd_opt(2020, 1, 1).expect("date")
    )]
    fn resolve_start_date_cases(
        #[case] latest_stored: Option<NaiveDate>,
        #[case] earliest: NaiveDate,
        #[case] expected: NaiveDate,
    ) {
        assert_eq!(resolve_start_date(latest_stored, earliest), expected);
    }

    #[backend_test_macros::database_test]
    async fn run_ingest_cycle_skips_when_no_manual_plan(db: crate::database::DatabaseHandle) {
        let mock = JQuantsMockServer::start().await;
        let client = mock.client().expect("client");
        let today = NaiveDate::from_ymd_opt(2024, 6, 1).expect("date");

        let (interest_stats, alert_stats) = run_ingest_cycle(&db, &client, today)
            .await
            .expect("cycle ok");

        assert_eq!(
            (interest_stats, alert_stats),
            (IngestStats::default(), IngestStats::default())
        );
    }

    #[backend_test_macros::database_test]
    async fn ingest_daily_fetches_past_the_former_per_cycle_cap(
        db: crate::database::DatabaseHandle,
    ) {
        let mock = JQuantsMockServer::start().await;
        let client = mock.client().expect("client");
        client.set_manual_plan(Some(JQuantsPlan::Standard));

        let start = NaiveDate::from_ymd_opt(2024, 1, 1).expect("date");
        let end = NaiveDate::from_ymd_opt(2024, 2, 10).expect("date"); // start から 40 日
        let target = NaiveDate::from_ymd_opt(2024, 2, 5).expect("date"); // start から 35 日 (旧上限 30 を超える)

        mock.margin_interest()
            .date("2024-02-05")
            .rows(vec![MockMarginInterestRow {
                date: "2024-02-05",
                code: "86970",
                iss_type: "1",
                shrt_vol: 100.0,
                long_vol: 200.0,
                shrt_neg_vol: 10.0,
                long_neg_vol: 20.0,
                shrt_std_vol: 90.0,
                long_std_vol: 180.0,
                shrt_val: Some(1000.0),
                long_val: Some(2000.0),
                shrt_neg_val: Some(100.0),
                long_neg_val: Some(200.0),
                shrt_std_val: Some(900.0),
                long_std_val: Some(1800.0),
            }])
            .ok()
            .await;

        let stats = ingest_daily(
            &db,
            &client,
            start,
            end,
            "テスト",
            MarginSource::fetch_margin_interest,
            margin_interest::upsert_margin_interest,
        )
        .await;

        assert_eq!(
            stats,
            IngestStats {
                days_fetched: 1,
                rows_upserted: 1,
            }
        );
        assert_eq!(
            margin_interest::find_latest_margin_interest_date(&db)
                .await
                .expect("query ok"),
            Some(target)
        );
    }
}
