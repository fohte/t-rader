//! 信用取引週末残高・日々公表信用取引残高を日次で取得し、DB に蓄積する定期タスク。
//!
//! IBKR には対応するデータが無いため DataProvider trait には追加せず、JQuantsClient を
//! 直接使う。契約プランが未設定の間は取り込まない (未設定時のレート制限は 5 req/min のため)。

use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use chrono::{Duration as ChronoDuration, NaiveDate, Utc};
use sea_orm::DatabaseConnection;
use tokio::task::JoinHandle;

use crate::data_provider::DataProviderError;
use crate::data_provider::DataProviderKind;
use crate::data_provider::jquants::JQuantsClient;
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

/// 1 サイクルで取得を試みる日数の上限。JQuantsClient の RateLimiter はウォッチリスト
/// 追加時の日足取得や sector_backfill と共有のため、大規模バックフィル時に専有しすぎ
/// ないよう抑える。上限に達した分は次サイクルに繰り越される。
const MAX_REQUESTS_PER_CYCLE: usize = 30;

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
/// 1 日分の取得・保存に失敗しても残りの日付は続行する。`MAX_REQUESTS_PER_CYCLE` に達したら
/// 打ち切り、残りは次サイクルに持ち越す。
async fn ingest_daily<'c, T, F, FetchFut, G, UpsertFut>(
    db: &'c DatabaseConnection,
    client: &'c JQuantsClient,
    start: NaiveDate,
    end: NaiveDate,
    label: &str,
    fetch: F,
    upsert: G,
) -> IngestStats
where
    F: Fn(&'c JQuantsClient, NaiveDate) -> FetchFut,
    FetchFut: Future<Output = Result<Vec<T>, DataProviderError>>,
    G: Fn(&'c DatabaseConnection, Vec<T>) -> UpsertFut,
    UpsertFut: Future<Output = Result<(), AppError>>,
{
    let mut stats = IngestStats::default();
    let mut date = start;
    let mut requests = 0usize;
    while date <= end && requests < MAX_REQUESTS_PER_CYCLE {
        requests += 1;
        match fetch(client, date).await {
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
/// 契約プランが未設定なら何もしない。
pub async fn run_ingest_cycle(
    db: &DatabaseConnection,
    client: &JQuantsClient,
    today: NaiveDate,
) -> Result<(IngestStats, IngestStats), AppError> {
    let Some(plan) = client.manual_plan() else {
        tracing::debug!("J-Quants 契約プラン未設定のため、信用残データの取り込みをスキップします");
        return Ok((IngestStats::default(), IngestStats::default()));
    };

    // 配信遅延を反映した契約上限日 (plan_to) を超えては取得できない (backfill.rs::latest_fetchable_date と同様)
    let (plan_from, plan_to) = plan.range(today);

    let interest_earliest = plan_from.max(margin_interest_start_date());
    let interest_latest = margin_interest::find_latest_margin_interest_date(db).await?;
    let interest_start = resolve_start_date(interest_latest, interest_earliest);
    let interest_stats = ingest_daily(
        db,
        client,
        interest_start,
        plan_to,
        "信用取引週末残高",
        JQuantsClient::fetch_margin_interest,
        margin_interest::upsert_margin_interest,
    )
    .await;

    let alert_earliest = plan_from.max(margin_alert_start_date());
    let alert_latest = margin_alert::find_latest_margin_alert_pub_date(db).await?;
    let alert_start = resolve_start_date(alert_latest, alert_earliest);
    let alert_stats = ingest_daily(
        db,
        client,
        alert_start,
        plan_to,
        "日々公表信用取引残高",
        JQuantsClient::fetch_margin_alert,
        margin_alert::upsert_margin_alert,
    )
    .await;

    Ok((interest_stats, alert_stats))
}

/// poll task を起動する。1 回目は即実行し、その後 `interval` で繰り返す。
pub fn spawn_poll(
    db: DatabaseConnection,
    provider: Arc<DataProviderKind>,
    interval: Duration,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let DataProviderKind::JQuants(client) = provider.as_ref() else {
            tracing::warn!("margin ingest には J-Quants provider が必要なため起動しません");
            return;
        };

        let mut ticker = tokio::time::interval(interval);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            ticker.tick().await;
            let today = Utc::now().date_naive();
            match run_ingest_cycle(&db, client, today).await {
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
    use rstest::rstest;
    use sqlx::PgPool;

    use super::*;
    use crate::data_provider::jquants::mock::JQuantsMockServer;
    use crate::testing::create_test_db;

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

    #[sqlx::test(migrations = false)]
    async fn run_ingest_cycle_skips_when_no_manual_plan(pool: PgPool) {
        let db = create_test_db(pool).await;
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
}
